#!/usr/bin/env bash
set -euo pipefail

# ====================================================================
# Gihomo — Arch Linux (.pkg.tar.zst) Packaging Script
# ====================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

APP_ID="art.artforge.Gihomo"
PKG_NAME="gihomo"
PKG_VERSION="1.1.0"
PKG_REL="1"

RAW_ARCH="${1:-${ARCH_TARGET:-$(uname -m)}}"
case "${RAW_ARCH}" in
    x86_64|amd64)
        ARCH="x86_64"
        NORM_ARCH="amd64"
        ;;
    aarch64|arm64|armv8*)
        ARCH="aarch64"
        NORM_ARCH="arm64"
        ;;
    *)
        ARCH="${RAW_ARCH}"
        NORM_ARCH="${RAW_ARCH}"
        ;;
esac

OUTPUT_DIR="${PROJECT_ROOT}/dist"
PKG_FILE="${PKG_NAME}-${PKG_VERSION}-${PKG_REL}-${ARCH}.pkg.tar.zst"
BUILD_DIR="${PROJECT_ROOT}/target/arch/${PKG_NAME}-${ARCH}"
OUTPUT_PKG="${OUTPUT_DIR}/${PKG_FILE}"

mkdir -p "${OUTPUT_DIR}"
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}/usr/bin"
mkdir -p "${BUILD_DIR}/usr/lib/gihomo/bin"
mkdir -p "${BUILD_DIR}/usr/share/applications"
mkdir -p "${BUILD_DIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${BUILD_DIR}/usr/share/glib-2.0/schemas"
mkdir -p "${BUILD_DIR}/usr/share/metainfo"
mkdir -p "${BUILD_DIR}/usr/share/polkit-1/rules.d"

if [ ! -f "target/release/gihomo" ]; then
    echo "==> 1. Building release binary (cargo build --release)..."
    cargo build --release
else
    echo "==> 1. Release binary target/release/gihomo already present, skipping cargo build."
fi

echo "==> 2. Ensuring Mihomo kernel for ${NORM_ARCH}..."
"${PROJECT_ROOT}/scripts/ensure-mihomo.sh" "${NORM_ARCH}"

echo "==> 3. Copying binaries and desktop assets..."
cp "target/release/gihomo" "${BUILD_DIR}/usr/bin/gihomo"
chmod 755 "${BUILD_DIR}/usr/bin/gihomo"

cp "assets/mihomo" "${BUILD_DIR}/usr/lib/gihomo/bin/mihomo"
chmod 755 "${BUILD_DIR}/usr/lib/gihomo/bin/mihomo"

sed -e "s|Icon=art.artforge.Gihomo|Icon=${APP_ID}|g" \
    "data/${APP_ID}.desktop.in" > "${BUILD_DIR}/usr/share/applications/${APP_ID}.desktop"
chmod 644 "${BUILD_DIR}/usr/share/applications/${APP_ID}.desktop"

cp data/icons/hicolor/scalable/apps/${APP_ID}*.svg \
   "${BUILD_DIR}/usr/share/icons/hicolor/scalable/apps/"
chmod 644 "${BUILD_DIR}"/usr/share/icons/hicolor/scalable/apps/${APP_ID}*.svg

cp "data/${APP_ID}.gschema.xml" \
   "${BUILD_DIR}/usr/share/glib-2.0/schemas/${APP_ID}.gschema.xml"
chmod 644 "${BUILD_DIR}/usr/share/glib-2.0/schemas/${APP_ID}.gschema.xml"

cp "data/${APP_ID}.metainfo.xml.in" \
   "${BUILD_DIR}/usr/share/metainfo/${APP_ID}.metainfo.xml"
chmod 644 "${BUILD_DIR}/usr/share/metainfo/${APP_ID}.metainfo.xml"

cp "data/${APP_ID}.rules" \
   "${BUILD_DIR}/usr/share/polkit-1/rules.d/${APP_ID}.rules"
chmod 644 "${BUILD_DIR}/usr/share/polkit-1/rules.d/${APP_ID}.rules"

echo "==> 4. Generating Arch package metadata (.PKGINFO and .INSTALL)..."
INSTALLED_SIZE=$(du -sb "${BUILD_DIR}" | awk '{print $1}')
BUILD_DATE=$(date +%s)

cat << EOF > "${BUILD_DIR}/.PKGINFO"
pkgname = ${PKG_NAME}
pkgbase = ${PKG_NAME}
pkgver = ${PKG_VERSION}-${PKG_REL}
pkgdesc = Modern native GTK4 + Libadwaita management client for Mihomo
url = https://github.com/zhanmq-artforge/gihomo
builddate = ${BUILD_DATE}
packager = zhanmq <zhanmq.china@gmail.com>
size = ${INSTALLED_SIZE}
arch = ${ARCH}
license = GPL-3.0-or-later
depend = gtk4
depend = libadwaita
depend = glib2
depend = libcap
optdepend = polkit: permissions for TUN mode and systemd-resolved
provides = gihomo
conflict = gihomo
EOF

cat << 'EOF' > "${BUILD_DIR}/.INSTALL"
post_install() {
    glib-compile-schemas /usr/share/glib-2.0/schemas 2>/dev/null || true
    update-desktop-database -q /usr/share/applications 2>/dev/null || true
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || true
    if command -v setcap >/dev/null 2>&1; then
        setcap cap_net_admin,cap_net_bind_service=+ep /usr/lib/gihomo/bin/mihomo 2>/dev/null || true
    fi
}

post_upgrade() {
    post_install
}

post_remove() {
    glib-compile-schemas /usr/share/glib-2.0/schemas 2>/dev/null || true
    update-desktop-database -q /usr/share/applications 2>/dev/null || true
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor 2>/dev/null || true
}
EOF

echo "==> 5. Creating Arch Linux package with zstd..."
(
    cd "${BUILD_DIR}"
    # Standard Arch .pkg.tar.zst ordering: metadata files first (.PKGINFO, .INSTALL), then directory tree
    tar --format=gnu --owner=0 --group=0 --numeric-owner \
        -cf - .PKGINFO .INSTALL usr \
        | zstd -c -T0 -19 > "${OUTPUT_PKG}"
)

echo "==> ✅ Arch Linux package successfully created: ${OUTPUT_PKG}"
ls -la "${OUTPUT_PKG}"
