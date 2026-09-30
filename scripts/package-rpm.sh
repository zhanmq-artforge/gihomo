#!/usr/bin/env bash
set -euo pipefail

# ====================================================================
# Gihomo — RedHat / Fedora (.rpm) Packaging Script
# ====================================================================

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

PKG_NAME="gihomo"
PKG_VERSION="1.0.0"
OUTPUT_DIR="${PROJECT_ROOT}/dist"
RPMBUILD_DIR="${PROJECT_ROOT}/target/rpmbuild"
SPEC_FILE="${PROJECT_ROOT}/packaging/rpm/gihomo.spec"

mkdir -p "${OUTPUT_DIR}"

if ! command -v rpmbuild &>/dev/null; then
    echo "==> ⚠️  'rpmbuild' not found on this system."
    echo "    On Ubuntu/Debian, install with:  sudo apt install -y rpm"
    echo "    On Fedora/RHEL, install with:   sudo dnf install -y rpm-build"
    echo "    Or you can build via 'cargo-generate-rpm' (pure Rust, no rpmbuild needed):"
    echo "      cargo install cargo-generate-rpm"
    echo "      cargo generate-rpm"
    exit 1
fi

echo "==> 1. Building release binary (cargo build --release)..."
cargo build --release

echo "==> 2. Preparing rpmbuild workspace in ${RPMBUILD_DIR}..."
rm -rf "${RPMBUILD_DIR}"
mkdir -p "${RPMBUILD_DIR}"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

cp "${SPEC_FILE}" "${RPMBUILD_DIR}/SPECS/"

echo "==> 3. Running rpmbuild..."
rpmbuild --define "_topdir ${RPMBUILD_DIR}" \
         --define "_builddir ${PROJECT_ROOT}" \
         -bb "${RPMBUILD_DIR}/SPECS/gihomo.spec"

echo "==> 4. Copying generated RPM to dist/..."
find "${RPMBUILD_DIR}/RPMS" -name "*.rpm" -exec cp {} "${OUTPUT_DIR}/" \;

echo "==> ✅ RPM package successfully created in: ${OUTPUT_DIR}"
ls -la "${OUTPUT_DIR}"/*.rpm
