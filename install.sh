#!/usr/bin/env sh
# Builds the dofustabs CLI in release mode, installs the binary for the
# current user, and runs `dofustabs install` to set up the Hyprland module,
# the bar widget and the icons. Re-run to upgrade (idempotent); pass
# --uninstall to reverse everything, including removing the binary.
#
# Note: ~/.local/bin/dofustabs previously held the old Tauri GUI binary from
# this project's earlier design; replacing it here is intentional.
set -eu

cd "$(dirname "$0")"

BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="$DATA_DIR/applications"
ICON_DIR="$DATA_DIR/icons/hicolor"

if [ "${1:-}" = "--uninstall" ]; then
  if [ -x "$BIN_DIR/dofustabs" ]; then
    "$BIN_DIR/dofustabs" uninstall
  fi
  rm -f "$BIN_DIR/dofustabs"
  # Remove the desktop entry and icons the old Tauri GUI installer created.
  rm -f "$APP_DIR/dofustabs.desktop"
  for size in 32 128 256 512; do
    rm -f "$ICON_DIR/${size}x${size}/apps/dofustabs.png"
  done
  update-desktop-database "$APP_DIR" 2>/dev/null || true
  echo "DofusTabs uninstalled."
  exit 0
fi

cargo build --release -p dofustabs-cli

install -Dm755 target/release/dofustabs "$BIN_DIR/dofustabs"
"$BIN_DIR/dofustabs" install

echo "DofusTabs installed: $BIN_DIR/dofustabs"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) echo "Note: $BIN_DIR is not in PATH." ;;
esac
