#!/usr/bin/env bash
set -euo pipefail

# ====================================================================
# Gihomo — RedHat / Fedora (.rpm) Packaging Script
# ====================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

PKG_NAME="gihomo"
PKG_VERSION="${PKG_VERSION:-$(grep -m 1 '^version = ' Cargo.toml | cut -d '"' -f 2)}"
OUTPUT_DIR="${PROJECT_ROOT}/dist"
RPMBUILD_DIR="${PROJECT_ROOT}/target/rpmbuild"
SPEC_FILE="${PROJECT_ROOT}/packaging/rpm/gihomo.spec"

RAW_ARCH="${1:-${RPM_ARCH:-$(uname -m)}}"
case "${RAW_ARCH}" in
    x86_64|amd64)
        RPM_ARCH="x86_64"
        NORM_ARCH="amd64"
        ;;
    aarch64|arm64|armv8*)
        RPM_ARCH="aarch64"
        NORM_ARCH="arm64"
        ;;
    *)
        RPM_ARCH="${RAW_ARCH}"
        NORM_ARCH="${RAW_ARCH}"
        ;;
esac

mkdir -p "${OUTPUT_DIR}"

if [ ! -f "target/release/gihomo" ]; then
    echo "==> 1. Building release binary (cargo build --release)..."
    cargo build --release
else
    echo "==> 1. Release binary target/release/gihomo already present, skipping cargo build."
fi

echo "==> 1.1 Ensuring Mihomo kernel for ${NORM_ARCH}..."
"${PROJECT_ROOT}/scripts/ensure-mihomo.sh" "${NORM_ARCH}"

if command -v rpmbuild &>/dev/null; then
    echo "==> 2. Preparing rpmbuild workspace in ${RPMBUILD_DIR}..."
    rm -rf "${RPMBUILD_DIR}"
    mkdir -p "${RPMBUILD_DIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

    cp "${SPEC_FILE}" "${RPMBUILD_DIR}/SPECS/"

    echo "==> 3. Running rpmbuild for ${RPM_ARCH}..."
    rpmbuild --define "_topdir ${RPMBUILD_DIR}" \
             --define "_builddir ${PROJECT_ROOT}" \
             --target "${RPM_ARCH}" \
             -bb "${RPMBUILD_DIR}/SPECS/gihomo.spec"

    echo "==> 4. Copying generated RPM to dist/..."
    find "${RPMBUILD_DIR}/RPMS" -name "*.rpm" -exec cp {} "${OUTPUT_DIR}/" \;
elif command -v cargo-generate-rpm &>/dev/null; then
    echo "==> 2. Using cargo-generate-rpm to build RPM package..."
    cargo generate-rpm --target-arch "${RPM_ARCH}"
    find "target/generate-rpm" -name "*.rpm" -exec cp {} "${OUTPUT_DIR}/" \;
else
    echo "==> ⚠️  Neither 'rpmbuild' nor 'cargo-generate-rpm' found."
    echo "    On Ubuntu/Debian:  sudo apt install -y rpm"
    echo "    On Fedora/RHEL:    sudo dnf install -y rpm-build"
    echo "    Or install cargo-generate-rpm: cargo install cargo-generate-rpm"
    exit 1
fi

echo "==> ✅ RPM package successfully created in: ${OUTPUT_DIR}"
ls -la "${OUTPUT_DIR}"/*.rpm

