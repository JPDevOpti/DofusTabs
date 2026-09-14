use serde::{Deserialize, Serialize};

/// Which GPU a game client renders on, as far as the platform can tell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuKind {
    /// The client opened a render node owned by a discrete GPU.
    Dedicated,
    /// The client only opened render nodes of a non-discrete (integrated) GPU.
    Integrated,
    /// Nothing opened yet, or the platform cannot inspect the process.
    #[default]
    Unknown,
}

impl GpuKind {
    /// Stable textual form used on the IPC contract.
    pub fn wire_name(self) -> &'static str {
        match self {
            GpuKind::Dedicated => "dedicated",
            GpuKind::Integrated => "integrated",
            GpuKind::Unknown => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_are_lowercase_and_match_serde() {
        for kind in [GpuKind::Dedicated, GpuKind::Integrated, GpuKind::Unknown] {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.wire_name()));
        }
    }

    #[test]
    fn default_is_unknown() {
        assert_eq!(GpuKind::default(), GpuKind::Unknown);
    }
}
