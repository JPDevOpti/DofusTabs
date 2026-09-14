#!/usr/bin/env sh
# Starts DofusTabs in development mode.
# Frontend changes reload instantly (Vite HMR); Rust changes trigger an
# automatic rebuild and relaunch of the app.
set -eu

cd "$(dirname "$0")"

if [ ! -d node_modules ]; then
  npm install
fi

exec npm run tauri dev -- "$@"
