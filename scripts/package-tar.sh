#!/usr/bin/env bash
set -euo pipefail

# ====================================================================
# Gihomo — Portable Tarball (.tar.gz) Packaging Script
# ====================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

APP_ID="art.artforge.Gihomo"
PKG_NAME="gihomo"
PKG_VERSION="1.1.0"

RAW_ARCH="${1:-${TARGET_ARCH:-$(uname -m)}}"
case "${RAW_ARCH}" in
    x86_64|amd64)
        NORM_ARCH="amd64"
        ;;
    aarch64|arm64|armv8*)
        NORM_ARCH="arm64"
        ;;
    *)
        NORM_ARCH="${RAW_ARCH}"
        ;;
esac

OUTPUT_DIR="${PROJECT_ROOT}/dist"
TAR_NAME="${PKG_NAME}-${PKG_VERSION}-linux-${NORM_ARCH}"
BUNDLE_DIR="${PROJECT_ROOT}/target/tarball/${TAR_NAME}"
OUTPUT_TAR="${OUTPUT_DIR}/${TAR_NAME}.tar.gz"

mkdir -p "${OUTPUT_DIR}"
rm -rf "${BUNDLE_DIR}"
mkdir -p "${BUNDLE_DIR}/bin"
mkdir -p "${BUNDLE_DIR}/share/applications"
mkdir -p "${BUNDLE_DIR}/share/icons/hicolor/scalable/apps"
mkdir -p "${BUNDLE_DIR}/share/glib-2.0/schemas"
mkdir -p "${BUNDLE_DIR}/share/metainfo"
mkdir -p "${BUNDLE_DIR}/share/polkit-1/rules.d"

if [ ! -f "target/release/gihomo" ]; then
    echo "==> 1. Building release binary (cargo build --release)..."
    cargo build --release
else
    echo "==> 1. Release binary target/release/gihomo already present, skipping cargo build."
fi

echo "==> 2. Ensuring Mihomo kernel for ${NORM_ARCH}..."
"${PROJECT_ROOT}/scripts/ensure-mihomo.sh" "${NORM_ARCH}"

echo "==> 3. Copying binaries and desktop assets..."
cp "target/release/gihomo" "${BUNDLE_DIR}/bin/gihomo"
chmod 755 "${BUNDLE_DIR}/bin/gihomo"

cp "assets/mihomo" "${BUNDLE_DIR}/bin/mihomo"
chmod 755 "${BUNDLE_DIR}/bin/mihomo"

sed -e "s|Icon=art.artforge.Gihomo|Icon=${APP_ID}|g" \
    "data/${APP_ID}.desktop.in" > "${BUNDLE_DIR}/share/applications/${APP_ID}.desktop"
chmod 644 "${BUNDLE_DIR}/share/applications/${APP_ID}.desktop"

cp data/icons/hicolor/scalable/apps/${APP_ID}*.svg \
   "${BUNDLE_DIR}/share/icons/hicolor/scalable/apps/"
chmod 644 "${BUNDLE_DIR}"/share/icons/hicolor/scalable/apps/${APP_ID}*.svg

cp "data/${APP_ID}.gschema.xml" \
   "${BUNDLE_DIR}/share/glib-2.0/schemas/${APP_ID}.gschema.xml"
chmod 644 "${BUNDLE_DIR}/share/glib-2.0/schemas/${APP_ID}.gschema.xml"

cp "data/${APP_ID}.metainfo.xml.in" \
   "${BUNDLE_DIR}/share/metainfo/${APP_ID}.metainfo.xml"
chmod 644 "${BUNDLE_DIR}/share/metainfo/${APP_ID}.metainfo.xml"

cp "data/${APP_ID}.rules" \
   "${BUNDLE_DIR}/share/polkit-1/rules.d/${APP_ID}.rules"
chmod 644 "${BUNDLE_DIR}/share/polkit-1/rules.d/${APP_ID}.rules"

[ -f "README.md" ] && cp "README.md" "${BUNDLE_DIR}/"
[ -f "LICENSE" ] && cp "LICENSE" "${BUNDLE_DIR}/"

# Create a self-contained install.sh
cat << 'EOF' > "${BUNDLE_DIR}/install.sh"
#!/usr/bin/env bash
set -e

PREFIX="${PREFIX:-/usr/local}"

echo "==> Installing Gihomo to ${PREFIX}..."
install -d "${PREFIX}/bin"
install -m 755 bin/gihomo "${PREFIX}/bin/gihomo"

install -d "${PREFIX}/lib/gihomo/bin"
install -m 755 bin/mihomo "${PREFIX}/lib/gihomo/bin/mihomo"

# Setcap for TUN mode if root
if [ "$(id -u)" -eq 0 ] && command -v setcap >/dev/null 2>&1; then
    setcap cap_net_admin,cap_net_bind_service=+ep "${PREFIX}/lib/gihomo/bin/mihomo" 2>/dev/null || true
fi

install -d "${PREFIX}/share/applications"
install -m 644 share/applications/*.desktop "${PREFIX}/share/applications/"

install -d "${PREFIX}/share/icons/hicolor/scalable/apps"
install -m 644 share/icons/hicolor/scalable/apps/*.svg "${PREFIX}/share/icons/hicolor/scalable/apps/"

install -d "${PREFIX}/share/glib-2.0/schemas"
install -m 644 share/glib-2.0/schemas/*.gschema.xml "${PREFIX}/share/glib-2.0/schemas/"

install -d "${PREFIX}/share/metainfo"
install -m 644 share/metainfo/*.metainfo.xml "${PREFIX}/share/metainfo/"

if [ "$(id -u)" -eq 0 ]; then
    install -d "/etc/polkit-1/rules.d"
    install -m 644 share/polkit-1/rules.d/*.rules "/etc/polkit-1/rules.d/" 2>/dev/null || true
fi

# Post-install cache updates
if command -v glib-compile-schemas >/dev/null 2>&1; then
    glib-compile-schemas "${PREFIX}/share/glib-2.0/schemas" 2>/dev/null || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${PREFIX}/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${PREFIX}/share/icons/hicolor" 2>/dev/null || true
fi

echo "==> ✅ Gihomo installed successfully! You can launch it with 'gihomo'."
EOF
chmod 755 "${BUNDLE_DIR}/install.sh"

echo "==> 4. Archiving tarball to ${OUTPUT_TAR}..."
tar -czvf "${OUTPUT_TAR}" -C "${PROJECT_ROOT}/target/tarball" "${TAR_NAME}"

echo "==> ✅ Tarball successfully created: ${OUTPUT_TAR}"
ls -la "${OUTPUT_TAR}"
