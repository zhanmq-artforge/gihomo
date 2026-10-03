#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

RAW_ARCH="${1:-${PKG_ARCH:-$(uname -m)}}"

case "${RAW_ARCH}" in
    x86_64|amd64)
        NORM_ARCH="amd64"
        MIHOMO_ARCH="linux-amd64"
        FILE_KEYWORD="x86-64"
        ;;
    aarch64|arm64|armv8*)
        NORM_ARCH="arm64"
        MIHOMO_ARCH="linux-arm64"
        FILE_KEYWORD="aarch64"
        ;;
    *)
        echo "==> ⚠️ Unsupported architecture for Mihomo bundling: ${RAW_ARCH}"
        exit 1
        ;;
esac

MIHOMO_DEST="${PROJECT_ROOT}/assets/mihomo"
mkdir -p "${PROJECT_ROOT}/assets"

# Check if existing assets/mihomo matches the target architecture
if [ -f "${MIHOMO_DEST}" ]; then
    if file "${MIHOMO_DEST}" | grep -q "${FILE_KEYWORD}"; then
        echo "==> ✅ Existing cached Mihomo matches target architecture (${NORM_ARCH})"
        chmod 755 "${MIHOMO_DEST}"
        exit 0
    else
        echo "==> ℹ️ Existing Mihomo does not match target architecture (${NORM_ARCH}), re-fetching..."
        rm -f "${MIHOMO_DEST}"
    fi
fi

# Also check system installed / user cache if on matching host
if [ "$(uname -m)" = "x86_64" ] && [ "${NORM_ARCH}" = "amd64" ] && [ -f "/usr/lib/gihomo/bin/mihomo" ]; then
    echo "==> Using system installed Mihomo from /usr/lib/gihomo/bin/mihomo"
    cp "/usr/lib/gihomo/bin/mihomo" "${MIHOMO_DEST}"
    chmod 755 "${MIHOMO_DEST}"
    exit 0
fi

MIHOMO_VER="v1.19.31"
DOWNLOAD_URL="https://github.com/MetaCubeX/mihomo/releases/download/${MIHOMO_VER}/mihomo-${MIHOMO_ARCH}-${MIHOMO_VER}.gz"
MIRROR_URL="https://ghproxy.net/${DOWNLOAD_URL}"

echo "==> Downloading Mihomo kernel (${NORM_ARCH}, version: ${MIHOMO_VER})..."
TMP_GZ=$(mktemp)

if ! curl -fsSL --connect-timeout 10 -o "${TMP_GZ}" "${DOWNLOAD_URL}"; then
    echo "    Primary download failed, trying mirror..."
    curl -fsSL --connect-timeout 15 -o "${TMP_GZ}" "${MIRROR_URL}"
fi

gzip -d -c "${TMP_GZ}" > "${MIHOMO_DEST}"
chmod 755 "${MIHOMO_DEST}"
rm -f "${TMP_GZ}"

echo "==> ✅ Mihomo kernel (${NORM_ARCH}) ready at: ${MIHOMO_DEST}"
