//! `launch`: starts the Ankama launcher on the dedicated GPU.

use std::path::PathBuf;

use serde::Deserialize;

use crate::paths;

#[derive(Debug, Default, Deserialize)]
struct CliConfig {
    launcher_path: Option<String>,
}

fn default_launcher_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));
    home.join("Applications/Dofus.AppImage")
}

fn resolve_launcher_path() -> PathBuf {
    let config_path = paths::cli_config_json_path();
    let configured = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|json| serde_json::from_str::<CliConfig>(&json).ok())
        .and_then(|c| c.launcher_path)
        .filter(|p| !p.trim().is_empty());

    match configured {
        Some(p) => PathBuf::from(p),
        None => default_launcher_path(),
    }
}

pub fn run() -> anyhow::Result<()> {
    let launcher = resolve_launcher_path();
    let backend = dofustabs_platform::linux_backend()?;
    backend.launcher.launch(&launcher)?;
    println!("launched {}", launcher.display());
    Ok(())
}
