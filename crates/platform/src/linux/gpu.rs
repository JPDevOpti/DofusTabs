//! GPU detection through the DRM nodes a process has open: `/proc/<pid>/fd`
//! links pointing at `/dev/dri/renderD*` or `card*` are mapped to their PCI
//! device via `/dev/dri/by-path`, whose vendor id lives in sysfs.

use crate::{GpuInspector, PlatformError};
use dofustabs_core::gpu::GpuKind;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// PCI vendor id of NVIDIA; the only vendor treated as a discrete GPU.
const NVIDIA_VENDOR: u32 = 0x10de;

/// Inspects processes through `/proc`, `/dev/dri` and sysfs. The roots are
/// injectable so tests can point it at a fixture tree.
pub struct LinuxGpuInspector {
    proc_root: PathBuf,
    dri_root: PathBuf,
    pci_devices_root: PathBuf,
}

impl LinuxGpuInspector {
    pub fn new() -> Self {
        Self::with_roots("/proc", "/dev/dri", "/sys/bus/pci/devices")
    }

    pub fn with_roots(
        proc_root: impl Into<PathBuf>,
        dri_root: impl Into<PathBuf>,
        pci_devices_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            proc_root: proc_root.into(),
            dri_root: dri_root.into(),
            pci_devices_root: pci_devices_root.into(),
        }
    }

    /// Names of the DRM nodes (`renderD128`, `card1`, ...) `pid` has open.
    fn opened_drm_nodes(&self, pid: u32) -> Result<Vec<String>, PlatformError> {
        let fd_dir = self.proc_root.join(pid.to_string()).join("fd");
        let entries = match fs::read_dir(&fd_dir) {
            Ok(entries) => entries,
            // The process is gone (or not visible): nothing to classify.
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };

        let mut nodes = Vec::new();
        for entry in entries.flatten() {
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            if let Some(name) = drm_node_name(&target, &self.dri_root) {
                if !nodes.contains(&name) {
                    nodes.push(name);
                }
            }
        }
        Ok(nodes)
    }

    /// Map from DRM node name to PCI address, built from `/dev/dri/by-path`.
    fn node_to_pci(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        let Ok(entries) = fs::read_dir(self.dri_root.join("by-path")) else {
            return map;
        };
        for entry in entries.flatten() {
            let link_name = entry.file_name().to_string_lossy().into_owned();
            let Some(address) = pci_address_from_by_path(&link_name) else {
                continue;
            };
            let Ok(target) = fs::read_link(entry.path()) else {
                continue;
            };
            let Some(node) = target.file_name() else {
                continue;
            };
            map.insert(node.to_string_lossy().into_owned(), address);
        }
        map
    }

    fn vendor_of(&self, pci_address: &str) -> Option<u32> {
        let raw =
            fs::read_to_string(self.pci_devices_root.join(pci_address).join("vendor")).ok()?;
        parse_vendor(&raw)
    }
}

impl Default for LinuxGpuInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuInspector for LinuxGpuInspector {
    fn gpu_of(&self, pid: u32) -> Result<GpuKind, PlatformError> {
        let nodes = self.opened_drm_nodes(pid)?;
        if nodes.is_empty() {
            return Ok(GpuKind::Unknown);
        }
        let by_node = self.node_to_pci();
        let vendors: Vec<u32> = nodes
            .iter()
            .filter_map(|node| by_node.get(node))
            .filter_map(|address| self.vendor_of(address))
            .collect();
        Ok(classify(&vendors))
    }
}

/// Dedicated as soon as any opened node belongs to NVIDIA; Integrated when
/// nodes were opened but none is NVIDIA; Unknown when nothing resolved.
fn classify(vendors: &[u32]) -> GpuKind {
    if vendors.contains(&NVIDIA_VENDOR) {
        GpuKind::Dedicated
    } else if vendors.is_empty() {
        GpuKind::Unknown
    } else {
        GpuKind::Integrated
    }
}

/// `renderD128` / `card1` when `target` is such a node under `dri_root`.
fn drm_node_name(target: &Path, dri_root: &Path) -> Option<String> {
    if target.parent()? != dri_root {
        return None;
    }
    let name = target.file_name()?.to_str()?;
    let is_node = |prefix: &str| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
    };
    (is_node("renderD") || is_node("card")).then(|| name.to_string())
}

/// `pci-0000:01:00.0-render` -> `0000:01:00.0` (also for `-card` links).
fn pci_address_from_by_path(link_name: &str) -> Option<String> {
    let rest = link_name.strip_prefix("pci-")?;
    let address = rest
        .strip_suffix("-render")
        .or_else(|| rest.strip_suffix("-card"))?;
    (!address.is_empty()).then(|| address.to_string())
}

/// sysfs vendor files read `0x10de\n`.
fn parse_vendor(raw: &str) -> Option<u32> {
    let trimmed = raw.trim();
    let hex = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    u32::from_str_radix(hex, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    /// Fixture tree mimicking the hybrid laptop layout: NVIDIA at
    /// 0000:01:00.0 (renderD128/card1), AMD iGPU at 0000:65:00.0
    /// (renderD129/card0).
    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> Self {
            let root = std::env::temp_dir()
                .join("dofustabs-gpu-tests")
                .join(format!("{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            let dri = root.join("dev/dri");
            fs::create_dir_all(dri.join("by-path")).unwrap();
            for node in ["renderD128", "renderD129", "card0", "card1"] {
                fs::write(dri.join(node), "").unwrap();
            }
            for (link, node) in [
                ("pci-0000:01:00.0-render", "renderD128"),
                ("pci-0000:01:00.0-card", "card1"),
                ("pci-0000:65:00.0-render", "renderD129"),
                ("pci-0000:65:00.0-card", "card0"),
            ] {
                symlink(format!("../{node}"), dri.join("by-path").join(link)).unwrap();
            }
            for (address, vendor) in [("0000:01:00.0", "0x10de\n"), ("0000:65:00.0", "0x1002\n")] {
                let dev = root.join("sys/bus/pci/devices").join(address);
                fs::create_dir_all(&dev).unwrap();
                fs::write(dev.join("vendor"), vendor).unwrap();
            }
            Self { root }
        }

        fn process(&self, pid: u32, fds: &[(&str, &str)]) {
            let fd_dir = self.root.join("proc").join(pid.to_string()).join("fd");
            fs::create_dir_all(&fd_dir).unwrap();
            for (fd, target) in fds {
                symlink(target, fd_dir.join(fd)).unwrap();
            }
        }

        fn inspector(&self) -> LinuxGpuInspector {
            LinuxGpuInspector::with_roots(
                self.root.join("proc"),
                self.root.join("dev/dri"),
                self.root.join("sys/bus/pci/devices"),
            )
        }

        fn dri(&self, node: &str) -> String {
            self.root
                .join("dev/dri")
                .join(node)
                .to_string_lossy()
                .into_owned()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn nvidia_render_node_means_dedicated() {
        let fx = Fixture::new("dedicated");
        fx.process(
            100,
            &[
                ("0", "/dev/null"),
                ("7", &fx.dri("renderD129")),
                ("9", &fx.dri("renderD128")),
            ],
        );
        assert_eq!(fx.inspector().gpu_of(100).unwrap(), GpuKind::Dedicated);
    }

    #[test]
    fn only_integrated_nodes_means_integrated() {
        let fx = Fixture::new("integrated");
        fx.process(
            200,
            &[("3", &fx.dri("renderD129")), ("4", &fx.dri("card0"))],
        );
        assert_eq!(fx.inspector().gpu_of(200).unwrap(), GpuKind::Integrated);
    }

    #[test]
    fn no_drm_nodes_or_missing_process_means_unknown() {
        let fx = Fixture::new("unknown");
        fx.process(300, &[("0", "/dev/null"), ("1", "socket:[1234]")]);
        let inspector = fx.inspector();
        assert_eq!(inspector.gpu_of(300).unwrap(), GpuKind::Unknown);
        assert_eq!(inspector.gpu_of(999).unwrap(), GpuKind::Unknown);
    }

    #[test]
    fn unresolvable_nodes_stay_unknown() {
        let fx = Fixture::new("unresolvable");
        fx.process(400, &[("3", &fx.dri("renderD200"))]);
        assert_eq!(fx.inspector().gpu_of(400).unwrap(), GpuKind::Unknown);
    }

    #[test]
    fn helpers_parse_node_names_addresses_and_vendors() {
        let dri = Path::new("/dev/dri");
        assert_eq!(
            drm_node_name(Path::new("/dev/dri/renderD128"), dri).as_deref(),
            Some("renderD128")
        );
        assert_eq!(
            drm_node_name(Path::new("/dev/dri/card0"), dri).as_deref(),
            Some("card0")
        );
        assert_eq!(drm_node_name(Path::new("/dev/dri/by-path"), dri), None);
        assert_eq!(drm_node_name(Path::new("/dev/null"), dri), None);
        assert_eq!(drm_node_name(Path::new("/dev/dri/renderD"), dri), None);

        assert_eq!(
            pci_address_from_by_path("pci-0000:01:00.0-render").as_deref(),
            Some("0000:01:00.0")
        );
        assert_eq!(
            pci_address_from_by_path("pci-0000:65:00.0-card").as_deref(),
            Some("0000:65:00.0")
        );
        assert_eq!(pci_address_from_by_path("platform-foo-render"), None);

        assert_eq!(parse_vendor("0x10de\n"), Some(NVIDIA_VENDOR));
        assert_eq!(parse_vendor("1002"), Some(0x1002));
        assert_eq!(parse_vendor("nope"), None);

        assert_eq!(classify(&[0x1002, NVIDIA_VENDOR]), GpuKind::Dedicated);
        assert_eq!(classify(&[0x8086]), GpuKind::Integrated);
        assert_eq!(classify(&[]), GpuKind::Unknown);
    }
}
