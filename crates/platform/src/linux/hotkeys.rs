use crate::{HotkeyBackend, HotkeyHandler, HotkeyId, PlatformError};
use dofustabs_core::settings::Hotkey;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

// Linux-only module, so hardcoding the Linux O_NONBLOCK value avoids a libc
// dependency for a single constant.
const O_NONBLOCK: i32 = 0o4000;

/// Global hotkeys via Hyprland binds. Each bind runs `sh` to echo the hotkey's
/// wire name into a FIFO owned by this backend; a reader thread dispatches
/// those lines to the handler. A FIFO is used because it can be written from a
/// bind with plain shell built-ins — no socat/nc required on the user's system.
///
/// The XDG GlobalShortcuts portal is the compositor-agnostic replacement for
/// this mechanism and is the intended future upgrade.
pub struct HyprlandHotkeys {
    fifo_path: PathBuf,
    binds: HashMap<HotkeyId, BindSpec>,
    /// While true, the binds in `binds` are not installed in Hyprland.
    suspended: bool,
    handler: Arc<Mutex<Option<HotkeyHandler>>>,
    running: Arc<AtomicBool>,
    reader: Option<JoinHandle<()>>,
}

struct BindSpec {
    /// Legacy `bind` keyword value (Hyprland < 0.56, config-file parser).
    bind_value: String,
    /// Legacy `unbind` keyword value.
    unbind_value: String,
    /// Lua statement registering the bind (Hyprland >= 0.56, Lua config).
    lua_bind: String,
    /// Lua statement removing the bind.
    lua_unbind: String,
}

/// Hyprland 0.56 replaced the config-file parser with Lua: `hyprctl keyword
/// bind` is rejected with "keyword can't work with non-legacy parsers", and
/// binds must go through `hyprctl eval` with the `hl` API instead. Try Lua
/// first and fall back to the keyword form for older compositors.
fn bind(spec: &BindSpec) -> Result<(), PlatformError> {
    let lua_err = match super::run_hyprctl(&["eval", &spec.lua_bind]) {
        Ok(stdout) if stdout.trim() == "ok" => return Ok(()),
        Ok(stdout) => format!("eval {}: {}", spec.lua_bind, stdout.trim()),
        Err(err) => err.to_string(),
    };
    let stdout = super::run_hyprctl(&["keyword", "bind", &spec.bind_value]).map_err(|err| {
        PlatformError::CommandFailed(format!(
            "bind {} failed on both paths: lua eval: {lua_err}; legacy keyword: {err}",
            spec.bind_value
        ))
    })?;
    super::expect_ok(&format!("bind {}", spec.bind_value), &stdout).map_err(|err| {
        PlatformError::CommandFailed(format!(
            "bind {} failed on both paths: lua eval: {lua_err}; legacy keyword: {err}",
            spec.bind_value
        ))
    })
}

fn unbind(spec: &BindSpec) -> Result<(), PlatformError> {
    let lua_err = match super::run_hyprctl(&["eval", &spec.lua_unbind]) {
        Ok(stdout) if stdout.trim() == "ok" => return Ok(()),
        Ok(stdout) => format!("eval {}: {}", spec.lua_unbind, stdout.trim()),
        Err(err) => err.to_string(),
    };
    let stdout = super::run_hyprctl(&["keyword", "unbind", &spec.unbind_value]).map_err(|err| {
        PlatformError::CommandFailed(format!(
            "unbind {} failed on both paths: lua eval: {lua_err}; legacy keyword: {err}",
            spec.unbind_value
        ))
    })?;
    super::expect_ok(&format!("unbind {}", spec.unbind_value), &stdout).map_err(|err| {
        PlatformError::CommandFailed(format!(
            "unbind {} failed on both paths: lua eval: {lua_err}; legacy keyword: {err}",
            spec.unbind_value
        ))
    })
}

impl HyprlandHotkeys {
    pub fn new() -> Result<Self, PlatformError> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let fifo_path = dir.join("dofustabs.fifo");

        let _ = fs::remove_file(&fifo_path);
        let status = Command::new("mkfifo")
            .arg(&fifo_path)
            .status()
            .map_err(|e| PlatformError::CommandFailed(format!("failed to spawn mkfifo: {e}")))?;
        if !status.success() {
            return Err(PlatformError::CommandFailed(format!(
                "mkfifo {} failed",
                fifo_path.display()
            )));
        }

        let handler: Arc<Mutex<Option<HotkeyHandler>>> = Arc::new(Mutex::new(None));
        let running = Arc::new(AtomicBool::new(true));
        let reader = spawn_reader(
            fifo_path.clone(),
            Arc::clone(&handler),
            Arc::clone(&running),
        );

        Ok(Self {
            fifo_path,
            binds: HashMap::new(),
            suspended: false,
            handler,
            running,
            reader: Some(reader),
        })
    }
}

impl HotkeyBackend for HyprlandHotkeys {
    fn register(&mut self, id: HotkeyId, hotkey: &Hotkey) -> Result<(), PlatformError> {
        self.unregister(id)?;
        if hotkey.is_empty() {
            return Ok(());
        }

        let spec = BindSpec {
            bind_value: build_bind_value(hotkey, id, &self.fifo_path),
            unbind_value: build_unbind_value(hotkey),
            lua_bind: build_lua_bind(hotkey, id, &self.fifo_path),
            lua_unbind: build_lua_unbind(hotkey),
        };
        if !self.suspended {
            bind(&spec)?;
        }
        self.binds.insert(id, spec);
        Ok(())
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), PlatformError> {
        let Some(spec) = self.binds.remove(&id) else {
            return Ok(());
        };
        if self.suspended {
            return Ok(());
        }
        unbind(&spec)
    }

    fn set_handler(&mut self, handler: HotkeyHandler) {
        *self.handler.lock().unwrap() = Some(handler);
    }

    /// Hyprland consumes a bound combination before any client sees it, so a
    /// suspend must really remove the binds; gating dispatch would not let the
    /// capture widget observe the keys.
    fn set_suspended(&mut self, suspended: bool) -> Result<(), PlatformError> {
        if self.suspended == suspended {
            return Ok(());
        }
        self.suspended = suspended;

        let mut first_error = None;
        for spec in self.binds.values() {
            let result = if suspended { unbind(spec) } else { bind(spec) };
            // Keep going on failure so binds do not end up half-toggled;
            // report the first error afterwards.
            if let Err(err) = result {
                first_error.get_or_insert(err);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

impl Drop for HyprlandHotkeys {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);

        if !self.suspended {
            for spec in self.binds.values() {
                let _ = unbind(spec);
            }
        }

        // Unblock the reader thread: a non-blocking write open fails with
        // ENXIO when no reader is waiting, which is fine — it means the
        // thread already exited.
        if let Ok(mut fifo) = OpenOptions::new()
            .write(true)
            .custom_flags(O_NONBLOCK)
            .open(&self.fifo_path)
        {
            let _ = fifo.write_all(b"\n");
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let _ = fs::remove_file(&self.fifo_path);
    }
}

fn spawn_reader(
    path: PathBuf,
    handler: Arc<Mutex<Option<HotkeyHandler>>>,
    running: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::SeqCst) {
            // Blocks until a bind (or Drop) opens the FIFO for writing; EOF
            // arrives when all writers close, then the FIFO is reopened.
            let Ok(fifo) = File::open(&path) else { return };
            for line in BufReader::new(fifo).lines() {
                if !running.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(line) = line else { break };
                if let Some(id) = HotkeyId::from_wire(line.trim()) {
                    if let Some(handler) = handler.lock().unwrap().as_ref() {
                        handler(id);
                    }
                }
            }
        }
    })
}

/// Hyprland `bind` keyword value: `MODS,key,exec,command`. The exec command
/// must not contain commas, and the FIFO lives under XDG_RUNTIME_DIR, which
/// contains neither commas nor spaces.
fn build_bind_value(hotkey: &Hotkey, id: HotkeyId, fifo: &Path) -> String {
    format!(
        "{},{},exec,sh -c 'echo {} > {}'",
        hypr_mods(hotkey),
        hypr_key(hotkey),
        id.wire_name(),
        fifo.display()
    )
}

fn build_unbind_value(hotkey: &Hotkey) -> String {
    format!("{},{}", hypr_mods(hotkey), hypr_key(hotkey))
}

/// Lua combo string for the `hl` API: `ALT + SHIFT + TAB`.
fn lua_combo(hotkey: &Hotkey) -> String {
    let mods = hypr_mods(hotkey);
    let key = hotkey.key.to_uppercase();
    if mods.is_empty() {
        key
    } else {
        format!("{} + {}", mods.replace(' ', " + "), key)
    }
}

fn build_lua_bind(hotkey: &Hotkey, id: HotkeyId, fifo: &Path) -> String {
    // Binds stack in Hyprland, so drop any previous bind on the same combo
    // first (including leftovers from an instance that died without cleanup).
    let combo = super::lua_string(&lua_combo(hotkey));
    let wire_name = super::lua_string(&id.wire_name());
    let fifo_path = super::lua_string(&fifo.display().to_string());
    format!(
        "pcall(hl.unbind, \"{combo}\"); hl.bind(\"{combo}\", hl.dsp.exec_cmd(\"sh -c 'echo {wire_name} > {fifo_path}'\"))"
    )
}

fn build_lua_unbind(hotkey: &Hotkey) -> String {
    format!("hl.unbind(\"{}\")", super::lua_string(&lua_combo(hotkey)))
}

fn hypr_mods(hotkey: &Hotkey) -> String {
    let mut parts = Vec::new();
    if hotkey.ctrl {
        parts.push("CTRL");
    }
    if hotkey.alt {
        parts.push("ALT");
    }
    if hotkey.shift {
        parts.push("SHIFT");
    }
    if hotkey.super_key {
        parts.push("SUPER");
    }
    parts.join(" ")
}

fn hypr_key(hotkey: &Hotkey) -> String {
    hotkey.key.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mods_render_in_fixed_order() {
        assert_eq!(hypr_mods(&Hotkey::alt("Tab")), "ALT");
        assert_eq!(hypr_mods(&Hotkey::alt_shift("Tab")), "ALT SHIFT");
        assert_eq!(
            hypr_mods(&Hotkey::new(true, true, true, true, "k")),
            "CTRL ALT SHIFT SUPER"
        );
        assert_eq!(hypr_mods(&Hotkey::default()), "");
    }

    #[test]
    fn lua_bind_uses_hl_api_with_plus_separated_combo() {
        let fifo = Path::new("/run/user/1000/dofustabs.fifo");
        assert_eq!(
            build_lua_bind(&Hotkey::alt_shift("Tab"), HotkeyId::Previous, fifo),
            "pcall(hl.unbind, \"ALT + SHIFT + TAB\"); hl.bind(\"ALT + SHIFT + TAB\", hl.dsp.exec_cmd(\"sh -c 'echo previous > /run/user/1000/dofustabs.fifo'\"))"
        );
        assert_eq!(
            build_lua_unbind(&Hotkey::alt("Home")),
            "hl.unbind(\"ALT + HOME\")"
        );
        assert_eq!(
            lua_combo(&Hotkey::new(false, false, false, false, "f5")),
            "F5"
        );
    }

    #[test]
    fn bind_value_targets_the_fifo() {
        let value = build_bind_value(
            &Hotkey::alt("Tab"),
            HotkeyId::Next,
            Path::new("/run/user/1000/dofustabs.fifo"),
        );
        assert_eq!(
            value,
            "ALT,tab,exec,sh -c 'echo next > /run/user/1000/dofustabs.fifo'"
        );
    }

    #[test]
    fn instance_bind_value_carries_the_pid() {
        let value = build_bind_value(
            &Hotkey::new(true, false, false, false, "1"),
            HotkeyId::Instance(4832),
            Path::new("/run/user/1000/dofustabs.fifo"),
        );
        assert_eq!(
            value,
            "CTRL,1,exec,sh -c 'echo instance:4832 > /run/user/1000/dofustabs.fifo'"
        );
    }

    #[test]
    fn unbind_value_matches_bind_prefix() {
        assert_eq!(
            build_unbind_value(&Hotkey::alt_shift("Tab")),
            "ALT SHIFT,tab"
        );
    }
}
