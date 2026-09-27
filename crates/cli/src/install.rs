//! `install` / `uninstall`: idempotent setup of the Hyprland module, the bar
//! widget, the icons and the config skeleton.

use std::fs;
use std::process::Command;

use crate::accounts::{write_accounts_lua, Accounts};
use crate::assets;
use crate::paths;

pub fn install() -> anyhow::Result<()> {
    // 1. Hyprland module.
    let module_path = paths::dofustabs_module_path();
    if module_path
        .exists()
        .then(|| fs::read_to_string(&module_path).ok())
        .flatten()
        .as_deref()
        != Some(assets::HYPRLAND_MODULE)
    {
        paths::backup_if_exists(&module_path)?;
        if let Some(dir) = module_path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&module_path, assets::HYPRLAND_MODULE)?;
        println!("installed {}", module_path.display());
    } else {
        println!("{} already up to date", module_path.display());
    }

    // 2. Loader block in hyprland.lua.
    let hyprland_lua = paths::hyprland_lua_path();
    if hyprland_lua.exists() {
        let contents = fs::read_to_string(&hyprland_lua)?;
        if !contents.contains(assets::LOADER_BEGIN) {
            paths::backup_if_exists(&hyprland_lua)?;
            let updated = format!("{contents}\n{}", assets::LOADER_BLOCK);
            fs::write(&hyprland_lua, updated)?;
            println!("added loader block to {}", hyprland_lua.display());
        } else {
            println!("loader block already present in {}", hyprland_lua.display());
        }
    } else {
        eprintln!(
            "warning: {} does not exist; loader block not added",
            hyprland_lua.display()
        );
    }

    // 3. Bar widget.
    let widget_dir = paths::widget_dir();
    fs::create_dir_all(&widget_dir)?;
    write_if_changed(&widget_dir.join("manifest.json"), assets::WIDGET_MANIFEST)?;
    write_if_changed(&widget_dir.join("BarWidget.qml"), assets::WIDGET_QML)?;

    // 4. Icons.
    let icons_dir = paths::icons_dir();
    fs::create_dir_all(&icons_dir)?;
    for (name, bytes) in assets::ICONS {
        let path = icons_dir.join(name);
        if fs::read(&path).ok().as_deref() != Some(*bytes) {
            fs::write(&path, bytes)?;
        }
    }
    println!("installed icons to {}", icons_dir.display());

    // 5. accounts.json / accounts.lua skeleton.
    let accounts_json = paths::accounts_json_path();
    if !accounts_json.exists() {
        Accounts::default().save(&accounts_json)?;
        println!("created {}", accounts_json.display());
    }
    let accounts_lua = paths::accounts_lua_path();
    if !accounts_lua.exists() {
        let accounts = Accounts::load(&accounts_json)?;
        let primary_class = crate::config::read_primary_class(&paths::cli_config_json_path())?;
        write_accounts_lua(&accounts_lua, &accounts, primary_class.as_deref())?;
        println!("created {}", accounts_lua.display());
    }

    // 5a. Remove any identity.json/.lua left behind under ~/.config/dofustabs
    // from before they moved to the runtime dir, so a stale copy is never
    // read by mistake.
    for legacy in [
        paths::legacy_identity_json_path(),
        paths::legacy_identity_lua_path(),
    ] {
        if legacy.exists() {
            fs::remove_file(&legacy)?;
            println!("removed stale {}", legacy.display());
        }
    }

    // 5a-2. One-time cleanup of the retired background CPU limit's leftover
    // cache/lock files. Does not touch any running client's systemd scope:
    // those are harmless without a quota and disappear on their own when the
    // client exits.
    for leftover in [paths::throttle_cache_path(), paths::throttle_lock_path()] {
        if leftover.exists() {
            fs::remove_file(&leftover)?;
            println!("removed stale {}", leftover.display());
        }
    }

    // 5b. classes.json: one source of truth for the bar's class picker
    // (stage 3), generated from `crates/core`'s game classes so the widget
    // never hand-copies the list.
    write_classes_json()?;

    // 6. Register the widget in the bar layout, if not already there.
    if !bar_layout_has_widget()? {
        let status = Command::new("omarchy")
            .args([
                "bar",
                "put",
                "local.dofus-workspaces",
                "--after",
                "local.workspace-dots",
            ])
            .status();
        match status {
            Ok(s) if s.success() => println!("added widget to the bar layout"),
            Ok(s) => eprintln!("warning: 'omarchy bar put' exited with {s}"),
            Err(e) => eprintln!("warning: failed to run 'omarchy bar put': {e}"),
        }
    } else {
        println!("widget already present in the bar layout");
    }

    // 7. Reload Hyprland so the module (re)loads.
    match Command::new("hyprctl").arg("reload").status() {
        Ok(s) if s.success() => println!("hyprctl reload: ok"),
        Ok(s) => eprintln!("warning: hyprctl reload exited with {s}"),
        Err(e) => eprintln!("warning: failed to run hyprctl reload: {e}"),
    }

    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    // 0b. Mouse integration (best-effort: only touches what enable set up).
    if let Err(e) = crate::mouse::run_disable() {
        eprintln!("warning: failed to disable mouse integration: {e}");
    }

    // 1. Module.
    let module_path = paths::dofustabs_module_path();
    if module_path.exists() {
        paths::backup_if_exists(&module_path)?;
        fs::remove_file(&module_path)?;
        println!("removed {}", module_path.display());
    }

    // 2. Loader block.
    let hyprland_lua = paths::hyprland_lua_path();
    if hyprland_lua.exists() {
        let contents = fs::read_to_string(&hyprland_lua)?;
        if let Some(updated) = remove_loader_block(&contents) {
            paths::backup_if_exists(&hyprland_lua)?;
            fs::write(&hyprland_lua, updated)?;
            println!("removed loader block from {}", hyprland_lua.display());
        }
    }

    // 3. Widget from the bar layout and its files.
    if bar_layout_has_widget().unwrap_or(false) {
        remove_widget_from_shell_json()?;
    }
    let widget_dir = paths::widget_dir();
    if widget_dir.exists() {
        fs::remove_dir_all(&widget_dir)?;
        println!("removed {}", widget_dir.display());
    }

    // 4. Icons.
    let icons_dir = paths::icons_dir();
    if icons_dir.exists() {
        fs::remove_dir_all(&icons_dir)?;
        println!("removed {}", icons_dir.display());
    }

    // 5. classes.json.
    let classes_json = paths::classes_json_path();
    if classes_json.exists() {
        fs::remove_file(&classes_json)?;
        println!("removed {}", classes_json.display());
    }

    // accounts.json is intentionally kept.

    match Command::new("hyprctl").arg("reload").status() {
        Ok(s) if s.success() => println!("hyprctl reload: ok"),
        Ok(s) => eprintln!("warning: hyprctl reload exited with {s}"),
        Err(e) => eprintln!("warning: failed to run hyprctl reload: {e}"),
    }

    Ok(())
}

/// `~/.local/share/dofustabs/classes.json`: `[{ "name": "<canonical>", "icon":
/// "<absolute icon path>" }, ...]`, alphabetical by canonical name. The bar's
/// class picker (stage 3) reads this instead of hand-copying the class list
/// into QML.
fn write_classes_json() -> anyhow::Result<()> {
    let icons_dir = paths::icons_dir();
    let mut classes: Vec<&dofustabs_core::game_class::GameClass> =
        dofustabs_core::game_class::ALL.iter().collect();
    classes.sort_by_key(|c| c.canonical_name);

    let entries: Vec<serde_json::Value> = classes
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.canonical_name,
                "icon": icons_dir.join(c.icon_file).display().to_string(),
            })
        })
        .collect();

    let path = paths::classes_json_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(&entries)?;
    write_if_changed(&path, &json)?;
    println!("generated {}", path.display());
    Ok(())
}

fn write_if_changed(path: &std::path::Path, contents: &str) -> anyhow::Result<()> {
    if fs::read_to_string(path).ok().as_deref() != Some(contents) {
        fs::write(path, contents)?;
    }
    Ok(())
}

/// The `omarchy bar` CLI has no `remove` verb (only `use`, `reset`,
/// `defaults`, `position`, `transparent`, `put`, `move`, `set`), so removing
/// the widget from the layout is done by editing `shell.json` directly,
/// after a timestamped backup.
fn remove_widget_from_shell_json() -> anyhow::Result<()> {
    let path = paths::shell_json_path();
    if !path.exists() {
        return Ok(());
    }
    let contents = fs::read_to_string(&path)?;
    let mut value: serde_json::Value = serde_json::from_str(&contents)?;
    let mut changed = false;
    if let Some(sections) = value
        .pointer_mut("/bar/layout")
        .and_then(|v| v.as_object_mut())
    {
        for (_, entries) in sections.iter_mut() {
            if let Some(array) = entries.as_array_mut() {
                let before = array.len();
                array.retain(|entry| {
                    entry.get("id").and_then(|v| v.as_str()) != Some("local.dofus-workspaces")
                });
                if array.len() != before {
                    changed = true;
                }
            }
        }
    }
    if changed {
        paths::backup_if_exists(&path)?;
        fs::write(&path, serde_json::to_string_pretty(&value)?)?;
        println!("removed widget entry from {}", path.display());
    }
    Ok(())
}

fn bar_layout_has_widget() -> anyhow::Result<bool> {
    let path = paths::shell_json_path();
    if !path.exists() {
        return Ok(false);
    }
    let contents = fs::read_to_string(&path)?;
    Ok(contents.contains("local.dofus-workspaces"))
}

/// Removes the text between (and including) the `-- BEGIN dofustabs` /
/// `-- END dofustabs` markers. Returns `None` when the markers are not found
/// (nothing to remove).
pub fn remove_loader_block(contents: &str) -> Option<String> {
    let start = contents.find(assets::LOADER_BEGIN)?;
    let mut end = contents[start..].find(assets::LOADER_END)? + start + assets::LOADER_END.len();
    // Also swallow one trailing newline right after the END marker, so
    // removal does not leave a blank line behind.
    if contents[end..].starts_with('\n') {
        end += 1;
    }
    let mut result = contents[..start].to_string();
    result.push_str(&contents[end..]);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_and_then_removes_the_loader_block_cleanly() {
        let base = "require(\"hypr.monitors\")\n\n-- trailing personal config\n";
        let with_block = format!("{base}\n{}", assets::LOADER_BLOCK);
        assert!(with_block.contains(assets::LOADER_BEGIN));

        let removed = remove_loader_block(&with_block).unwrap();
        assert!(!removed.contains(assets::LOADER_BEGIN));
        assert!(!removed.contains(assets::LOADER_END));
        assert!(removed.contains("require(\"hypr.monitors\")"));
        assert!(removed.contains("trailing personal config"));
    }

    #[test]
    fn remove_loader_block_is_none_when_markers_absent() {
        assert_eq!(remove_loader_block("no markers here"), None);
    }

    #[test]
    fn remove_loader_block_leaves_surrounding_text_untouched_on_sample_config() {
        let sample = "line1\nline2\n-- BEGIN dofustabs\nfoo\nbar\n-- END dofustabs\nline3\n";
        let removed = remove_loader_block(sample).unwrap();
        assert_eq!(removed, "line1\nline2\nline3\n");
    }
}
