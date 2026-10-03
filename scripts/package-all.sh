#!/usr/bin/env bash
set -euo pipefail

# ====================================================================
# Gihomo — Master Package Script (All Formats)
# Packages: .deb, .rpm, .tar.gz, .pkg.tar.zst (Arch), .AppImage
# ====================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

RAW_ARCH="${1:-${PKG_ARCH:-$(uname -m)}}"
case "${RAW_ARCH}" in
    x86_64|amd64)
        DEB_ARCH="amd64"
        RPM_ARCH="x86_64"
        ARCH_ARCH="x86_64"
        TAR_ARCH="amd64"
        APPIMAGE_ARCH="x86_64"
        ;;
    aarch64|arm64|armv8*)
        DEB_ARCH="arm64"
        RPM_ARCH="aarch64"
        ARCH_ARCH="aarch64"
        TAR_ARCH="arm64"
        APPIMAGE_ARCH="aarch64"
        ;;
    *)
        echo "==> ⚠️ Unsupported architecture: ${RAW_ARCH}"
        exit 1
        ;;
esac

echo "======================================================================"
echo "==> Starting Complete Packaging for architecture: ${RAW_ARCH}"
echo "    Debian Arch:   ${DEB_ARCH}"
echo "    RPM Arch:      ${RPM_ARCH}"
echo "    ArchLinux:     ${ARCH_ARCH}"
echo "    Tarball Arch:  ${TAR_ARCH}"
echo "    AppImage Arch: ${APPIMAGE_ARCH}"
echo "======================================================================"

# Step 0: Ensure cargo build
if [ ! -f "target/release/gihomo" ]; then
    echo "==> Building release binary (cargo build --release)..."
    cargo build --release
fi

# Step 1: Ensure Mihomo kernel
"${PROJECT_ROOT}/scripts/ensure-mihomo.sh" "${DEB_ARCH}"

# Step 2: Build .deb package
echo ""
echo "==> [1/5] Building Debian (.deb) package..."
"${PROJECT_ROOT}/scripts/package-deb.sh" "${DEB_ARCH}"

# Step 3: Build .tar.gz portable bundle
echo ""
echo "==> [2/5] Building Portable Tarball (.tar.gz)..."
"${PROJECT_ROOT}/scripts/package-tar.sh" "${TAR_ARCH}"

# Step 4: Build Arch Linux (.pkg.tar.zst)
echo ""
echo "==> [3/5] Building Arch Linux (.pkg.tar.zst)..."
"${PROJECT_ROOT}/scripts/package-arch.sh" "${ARCH_ARCH}"

# Step 5: Build AppImage (.AppImage)
echo ""
echo "==> [4/5] Building AppImage (.AppImage)..."
"${PROJECT_ROOT}/scripts/package-appimage.sh" "${APPIMAGE_ARCH}"

# Step 6: Build RPM (.rpm) if tool available
echo ""
echo "==> [5/5] Building RPM (.rpm)..."
if command -v rpmbuild >/dev/null 2>&1 || command -v cargo-generate-rpm >/dev/null 2>&1; then
    "${PROJECT_ROOT}/scripts/package-rpm.sh" "${RPM_ARCH}" || echo "⚠️ RPM build failed or skipped"
else
    echo "⚠️ Skipping RPM build (neither 'rpmbuild' nor 'cargo-generate-rpm' found on host)."
fi

echo ""
echo "======================================================================"
echo "==> ✅ All available packages generated in: ${PROJECT_ROOT}/dist"
echo "======================================================================"
ls -la "${PROJECT_ROOT}/dist"
