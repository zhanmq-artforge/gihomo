#!/usr/bin/env bash
set -euo pipefail

# ====================================================================
# Gihomo — Debian (.deb) Packaging Script
# ====================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

APP_ID="art.artforge.Gihomo"
PKG_NAME="gihomo"
PKG_VERSION="1.1.0"

# Architecture detection: argument > environment variable > dpkg > uname
RAW_ARCH="${1:-${PKG_ARCH:-$(dpkg --print-architecture 2>/dev/null || uname -m)}}"
case "${RAW_ARCH}" in
    x86_64|amd64)
        PKG_ARCH="amd64"
        ;;
    aarch64|arm64|armv8*)
        PKG_ARCH="arm64"
        ;;
    *)
        PKG_ARCH="${RAW_ARCH}"
        ;;
esac

OUTPUT_DIR="${PROJECT_ROOT}/dist"
DEB_DIR="target/debian/${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}"
OUTPUT_DEB="${OUTPUT_DIR}/${PKG_NAME}_${PKG_VERSION}_${PKG_ARCH}.deb"
mkdir -p "${OUTPUT_DIR}"

if [ ! -f "target/release/gihomo" ]; then
    echo "==> 1. Building release binary (cargo build --release)..."
    cargo build --release
else
    echo "==> 1. Release binary target/release/gihomo already present, skipping cargo build."
fi

echo "==> 2. Preparing packaging directory structure in ${DEB_DIR}..."
rm -rf "${DEB_DIR}"
mkdir -p "${DEB_DIR}/DEBIAN"
mkdir -p "${DEB_DIR}/usr/bin"
mkdir -p "${DEB_DIR}/usr/lib/gihomo/bin"
mkdir -p "${DEB_DIR}/usr/share/applications"
mkdir -p "${DEB_DIR}/usr/share/icons/hicolor/scalable/apps"
mkdir -p "${DEB_DIR}/usr/share/glib-2.0/schemas"
mkdir -p "${DEB_DIR}/usr/share/metainfo"

echo "==> 3. Copying binary and assets..."
cp "target/release/gihomo" "${DEB_DIR}/usr/bin/gihomo"
chmod 755 "${DEB_DIR}/usr/bin/gihomo"

echo "==> 3.1 Bundling native Mihomo kernel into /usr/lib/gihomo/bin/mihomo..."
"${PROJECT_ROOT}/scripts/ensure-mihomo.sh" "${PKG_ARCH}"
KERNEL_DEST="${DEB_DIR}/usr/lib/gihomo/bin/mihomo"
cp "${PROJECT_ROOT}/assets/mihomo" "${KERNEL_DEST}"
chmod 755 "${KERNEL_DEST}"

# Desktop file
sed -e "s|Icon=art.artforge.Gihomo|Icon=${APP_ID}|g" \
    "data/${APP_ID}.desktop.in" > "${DEB_DIR}/usr/share/applications/${APP_ID}.desktop"
chmod 644 "${DEB_DIR}/usr/share/applications/${APP_ID}.desktop"

# Icons
cp data/icons/hicolor/scalable/apps/${APP_ID}*.svg \
   "${DEB_DIR}/usr/share/icons/hicolor/scalable/apps/"
chmod 644 "${DEB_DIR}"/usr/share/icons/hicolor/scalable/apps/${APP_ID}*.svg

# GSettings Schema
cp "data/${APP_ID}.gschema.xml" \
   "${DEB_DIR}/usr/share/glib-2.0/schemas/${APP_ID}.gschema.xml"
chmod 644 "${DEB_DIR}/usr/share/glib-2.0/schemas/${APP_ID}.gschema.xml"

# AppStream Metainfo
cp "data/${APP_ID}.metainfo.xml.in" \
   "${DEB_DIR}/usr/share/metainfo/${APP_ID}.metainfo.xml"
chmod 644 "${DEB_DIR}/usr/share/metainfo/${APP_ID}.metainfo.xml"

# Polkit rules for TUN mode & systemd-resolved
mkdir -p "${DEB_DIR}/usr/share/polkit-1/rules.d"
cp "data/${APP_ID}.rules" \
   "${DEB_DIR}/usr/share/polkit-1/rules.d/${APP_ID}.rules"
chmod 644 "${DEB_DIR}/usr/share/polkit-1/rules.d/${APP_ID}.rules"

echo "==> 4. Creating DEBIAN/control, postinst, and postrm..."

# Installed size in KB
INSTALLED_SIZE=$(du -sk "${DEB_DIR}/usr" | cut -f1)

cat <<EOF > "${DEB_DIR}/DEBIAN/control"
Package: ${PKG_NAME}
Version: ${PKG_VERSION}
Section: net
Priority: optional
Architecture: ${PKG_ARCH}
Installed-Size: ${INSTALLED_SIZE}
Maintainer: zhanmq <zhanmq.china@gmail.com>
Depends: libc6, libgtk-4-1, libadwaita-1-0, libglib2.0-0, libcap2-bin
Homepage: https://github.com/zhanmq-artforge/gihomo
Description: Modern native GTK4 + Libadwaita management client for Mihomo
 Gihomo is a modern native desktop client for Mihomo (Clash.Meta)
 running directly on Linux and GNOME environments.
 Features subscription management, node switching, GNOME system proxy
 integration, kernel TUN mode toggle, and real-time traffic monitoring.
EOF
chmod 644 "${DEB_DIR}/DEBIAN/control"

cat <<'EOF' > "${DEB_DIR}/DEBIAN/postinst"
#!/bin/sh
set -e

if [ "$1" = "configure" ]; then
    # Compile GSettings schemas
    if [ -x /usr/bin/glib-compile-schemas ]; then
        /usr/bin/glib-compile-schemas /usr/share/glib-2.0/schemas/ || true
    fi
    # Update desktop database
    if [ -x /usr/bin/update-desktop-database ]; then
        /usr/bin/update-desktop-database -q /usr/share/applications/ || true
    fi
    # Update icon cache
    if [ -x /usr/bin/gtk-update-icon-cache ]; then
        /usr/bin/gtk-update-icon-cache -q /usr/share/icons/hicolor/ || true
    fi
    # Grant TUN network privileges to Mihomo kernel if present
    if [ -x /usr/lib/gihomo/bin/mihomo ]; then
        setcap cap_net_admin,cap_net_bind_service=+ep /usr/lib/gihomo/bin/mihomo || true
    elif [ -x /usr/bin/mihomo ]; then
        setcap cap_net_admin,cap_net_bind_service=+ep /usr/bin/mihomo || true
    fi
fi

exit 0
EOF
chmod 755 "${DEB_DIR}/DEBIAN/postinst"

cat <<'EOF' > "${DEB_DIR}/DEBIAN/postrm"
#!/bin/sh
set -e

if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    if [ -x /usr/bin/glib-compile-schemas ]; then
        /usr/bin/glib-compile-schemas /usr/share/glib-2.0/schemas/ || true
    fi
    if [ -x /usr/bin/update-desktop-database ]; then
        /usr/bin/update-desktop-database -q /usr/share/applications/ || true
    fi
    if [ -x /usr/bin/gtk-update-icon-cache ]; then
        /usr/bin/gtk-update-icon-cache -q /usr/share/icons/hicolor/ || true
    fi
fi

exit 0
EOF
chmod 755 "${DEB_DIR}/DEBIAN/postrm"

echo "==> 5. Building Debian package with dpkg-deb..."
dpkg-deb --build --root-owner-group "${DEB_DIR}" "${OUTPUT_DEB}"

echo "==> ✅ Debian package successfully created at: ${OUTPUT_DEB}"
dpkg-deb --info "${OUTPUT_DEB}"
dpkg-deb --contents "${OUTPUT_DEB}"
