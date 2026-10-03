#!/usr/bin/env bash
set -euo pipefail

# ====================================================================
# Gihomo — AppImage Packaging Script
# ====================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

APP_ID="art.artforge.Gihomo"
PKG_NAME="Gihomo"
PKG_VERSION="1.1.0"

RAW_ARCH="${1:-${APPIMAGE_ARCH:-$(uname -m)}}"
case "${RAW_ARCH}" in
    x86_64|amd64)
        APPIMAGE_ARCH="x86_64"
        NORM_ARCH="amd64"
        ;;
    aarch64|arm64|armv8*)
        APPIMAGE_ARCH="aarch64"
        NORM_ARCH="arm64"
        ;;
    *)
        APPIMAGE_ARCH="${RAW_ARCH}"
        NORM_ARCH="${RAW_ARCH}"
        ;;
esac

OUTPUT_DIR="${PROJECT_ROOT}/dist"
APPDIR="${PROJECT_ROOT}/target/appimage/${PKG_NAME}.AppDir"
OUTPUT_APPIMAGE="${OUTPUT_DIR}/${PKG_NAME}-${PKG_VERSION}-${APPIMAGE_ARCH}.AppImage"

mkdir -p "${OUTPUT_DIR}"
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/lib/gihomo/bin"
mkdir -p "${APPDIR}/usr/share/applications"
mkdir -p "${APPDIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${APPDIR}/usr/share/glib-2.0/schemas"
mkdir -p "${APPDIR}/usr/share/metainfo"

if [ ! -f "target/release/gihomo" ]; then
    echo "==> 1. Building release binary (cargo build --release)..."
    cargo build --release
else
    echo "==> 1. Release binary target/release/gihomo already present, skipping cargo build."
fi

echo "==> 2. Ensuring Mihomo kernel for ${NORM_ARCH}..."
"${PROJECT_ROOT}/scripts/ensure-mihomo.sh" "${NORM_ARCH}"

echo "==> 3. Assembling AppDir structure..."
cp "target/release/gihomo" "${APPDIR}/usr/bin/gihomo"
chmod 755 "${APPDIR}/usr/bin/gihomo"

cp "assets/mihomo" "${APPDIR}/usr/lib/gihomo/bin/mihomo"
chmod 755 "${APPDIR}/usr/lib/gihomo/bin/mihomo"

sed -e "s|Icon=art.artforge.Gihomo|Icon=${APP_ID}|g" \
    "data/${APP_ID}.desktop.in" > "${APPDIR}/${APP_ID}.desktop"
cp "${APPDIR}/${APP_ID}.desktop" "${APPDIR}/usr/share/applications/${APP_ID}.desktop"
chmod 644 "${APPDIR}/${APP_ID}.desktop" "${APPDIR}/usr/share/applications/${APP_ID}.desktop"

cp "data/icons/hicolor/scalable/apps/${APP_ID}.svg" "${APPDIR}/${APP_ID}.svg"
cp data/icons/hicolor/scalable/apps/${APP_ID}*.svg \
   "${APPDIR}/usr/share/icons/hicolor/scalable/apps/"
chmod 644 "${APPDIR}/${APP_ID}.svg" "${APPDIR}"/usr/share/icons/hicolor/scalable/apps/${APP_ID}*.svg

cp "data/${APP_ID}.gschema.xml" \
   "${APPDIR}/usr/share/glib-2.0/schemas/${APP_ID}.gschema.xml"
if command -v glib-compile-schemas >/dev/null 2>&1; then
    glib-compile-schemas "${APPDIR}/usr/share/glib-2.0/schemas/"
fi

cp "data/${APP_ID}.metainfo.xml.in" \
   "${APPDIR}/usr/share/metainfo/${APP_ID}.metainfo.xml"

# Create standard AppRun entrypoint
cat << 'EOF' > "${APPDIR}/AppRun"
#!/bin/sh
set -e

HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="${HERE}/usr/bin:${HERE}/usr/lib/gihomo/bin:${PATH}"
export GSETTINGS_SCHEMA_DIR="${HERE}/usr/share/glib-2.0/schemas:${GSETTINGS_SCHEMA_DIR}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"

# Check for GTK4 Wayland / X11 display
exec "${HERE}/usr/bin/gihomo" "$@"
EOF
chmod 755 "${APPDIR}/AppRun"

echo "==> 4. Obtaining appimagetool for ${APPIMAGE_ARCH}..."
TOOL_CACHE="${PROJECT_ROOT}/target/tools"
mkdir -p "${TOOL_CACHE}"
APPIMAGETOOL="${TOOL_CACHE}/appimagetool-${APPIMAGE_ARCH}.AppImage"

if [ ! -f "${APPIMAGETOOL}" ]; then
    TOOL_URL="https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${APPIMAGE_ARCH}.AppImage"
    echo "    Downloading appimagetool from ${TOOL_URL}..."
    curl -fsSL -o "${APPIMAGETOOL}" "${TOOL_URL}" || \
    curl -fsSL -o "${APPIMAGETOOL}" "https://ghproxy.net/${TOOL_URL}"
    chmod +x "${APPIMAGETOOL}"
fi

echo "==> 5. Building AppImage..."
# Use --appimage-extract-and-run to ensure execution in FUSE-less containers/runners
export ARCH="${APPIMAGE_ARCH}"
"${APPIMAGETOOL}" --appimage-extract-and-run "${APPDIR}" "${OUTPUT_APPIMAGE}"

echo "==> ✅ AppImage successfully created: ${OUTPUT_APPIMAGE}"
ls -la "${OUTPUT_APPIMAGE}"
