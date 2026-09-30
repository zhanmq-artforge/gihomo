#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"
SCHEMA_DIR="${HOME}/.local/share/glib-2.0/schemas"
APP_ID="art.artforge.Gihomo"

echo "==> Building Gihomo in release mode..."
cd "${SCRIPT_DIR}"
cargo build --release

echo "==> Creating local directories..."
mkdir -p "${BIN_DIR}" "${APP_DIR}" "${ICON_DIR}" "${SCHEMA_DIR}"

echo "==> Installing binary..."
install -m 755 "${SCRIPT_DIR}/target/release/gihomo" "${BIN_DIR}/gihomo"

echo "==> Installing desktop and status icons..."
cp "${SCRIPT_DIR}"/data/icons/hicolor/scalable/apps/${APP_ID}*.svg "${ICON_DIR}/"

echo "==> Installing desktop entry..."
sed "s|Exec=gihomo|Exec=${BIN_DIR}/gihomo|" \
    "${SCRIPT_DIR}/data/${APP_ID}.desktop.in" > "${APP_DIR}/${APP_ID}.desktop"
chmod +x "${APP_DIR}/${APP_ID}.desktop"

echo "==> Installing GSettings schema..."
cp "${SCRIPT_DIR}/data/${APP_ID}.gschema.xml" "${SCHEMA_DIR}/"
glib-compile-schemas "${SCHEMA_DIR}"

echo "==> Updating system desktop & icon caches..."
update-desktop-database "${APP_DIR}" 2>/dev/null || true
gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" 2>/dev/null || true

echo "==> Installation complete! You can now launch 'Gihomo' from your application menu or run 'gihomo'."
