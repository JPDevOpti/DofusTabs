//! Files embedded in the binary at build time: the Hyprland module, the bar
//! widget, and the class icons. `install` writes these out; `doctor` compares
//! the installed copies against them byte-for-byte.

pub const HYPRLAND_MODULE: &str = include_str!("../../../hyprland/dofustabs.lua");

pub const WIDGET_MANIFEST: &str =
    include_str!("../../../omarchy/local.dofus-workspaces/manifest.json");
pub const WIDGET_QML: &str = include_str!("../../../omarchy/local.dofus-workspaces/BarWidget.qml");

/// `(file name, bytes)` for every class icon plus the generic fallback.
pub const ICONS: &[(&str, &[u8])] = &[
    (
        "Aniripsa.png",
        include_bytes!("../../../assets/icons/Aniripsa.png"),
    ),
    (
        "Anutrof.png",
        include_bytes!("../../../assets/icons/Anutrof.png"),
    ),
    ("Feca.png", include_bytes!("../../../assets/icons/Feca.png")),
    (
        "Forjalanza.png",
        include_bytes!("../../../assets/icons/Forjalanza.png"),
    ),
    (
        "Hipermago.png",
        include_bytes!("../../../assets/icons/Hipermago.png"),
    ),
    ("Ocra.png", include_bytes!("../../../assets/icons/Ocra.png")),
    (
        "Osamodas.png",
        include_bytes!("../../../assets/icons/Osamodas.png"),
    ),
    (
        "Pandawa.png",
        include_bytes!("../../../assets/icons/Pandawa.png"),
    ),
    (
        "Sacrogrito.png",
        include_bytes!("../../../assets/icons/Sacrogrito.png"),
    ),
    (
        "Sadida.png",
        include_bytes!("../../../assets/icons/Sadida.png"),
    ),
    (
        "Selotrop.png",
        include_bytes!("../../../assets/icons/Selotrop.png"),
    ),
    ("Sram.png", include_bytes!("../../../assets/icons/Sram.png")),
    (
        "Steamer.png",
        include_bytes!("../../../assets/icons/Steamer.png"),
    ),
    (
        "Tymador.png",
        include_bytes!("../../../assets/icons/Tymador.png"),
    ),
    (
        "Uginak.png",
        include_bytes!("../../../assets/icons/Uginak.png"),
    ),
    (
        "Xelor.png",
        include_bytes!("../../../assets/icons/Xelor.png"),
    ),
    (
        "Yopuka.png",
        include_bytes!("../../../assets/icons/Yopuka.png"),
    ),
    (
        "Zobal.png",
        include_bytes!("../../../assets/icons/Zobal.png"),
    ),
    (
        "Zurcar.png",
        include_bytes!("../../../assets/icons/Zurcar.png"),
    ),
    (
        "default.png",
        include_bytes!("../../../assets/icons/default.png"),
    ),
];

/// The loader block inserted into `hyprland.lua`, reused verbatim as it was
/// already present on the target machine (canonical text per the design).
pub const LOADER_BLOCK: &str = r#"-- BEGIN dofustabs
do
  local path = os.getenv("HOME") .. "/.config/hypr/dofustabs.lua"
  local ok, err = pcall(dofile, path)
  if not ok then
    local f = io.open(os.getenv("HOME") .. "/.cache/dofustabs.log", "a")
    if f then
      f:write(os.date("%Y-%m-%d %H:%M:%S") .. " ERROR: failed to load dofustabs.lua: " .. tostring(err) .. "\n")
      f:close()
    end
  end
end
-- END dofustabs
"#;

pub const LOADER_BEGIN: &str = "-- BEGIN dofustabs";
pub const LOADER_END: &str = "-- END dofustabs";
