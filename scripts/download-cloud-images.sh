#!/bin/bash
# Download cloud images for automated VM builds

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
source "$SCRIPT_DIR/../configs/defaults.env" 2>/dev/null || source "${REAGENTS_ROOT:-$(dirname "$SCRIPT_DIR")}/configs/defaults.env" 2>/dev/null || true
CLOUD_DIR="${REAGENTS_ROOT}/images/cloud"

mkdir -p "${CLOUD_DIR}"
cd "${CLOUD_DIR}"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Downloading Cloud Images (~2GB)                                     ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Ubuntu 24.04 Server Cloud Image
if [ ! -f "${CLOUD_IMAGE_UBUNTU_2404}" ]; then
    echo "📥 Downloading Ubuntu 24.04 Server Cloud Image (~700MB)..."
    wget -c --progress=bar:force:noscroll \
        -O "${CLOUD_IMAGE_UBUNTU_2404}" \
        https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
    echo "✅ Ubuntu 24.04 cloud image downloaded"
else
    echo "✓ Ubuntu 24.04 cloud image already exists"
fi

# Ubuntu 22.04 Server Cloud Image
if [ ! -f "${CLOUD_IMAGE_UBUNTU_2204}" ]; then
    echo "📥 Downloading Ubuntu 22.04 Server Cloud Image (~600MB)..."
    wget -c --progress=bar:force:noscroll \
        -O "${CLOUD_IMAGE_UBUNTU_2204}" \
        https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img
    echo "✅ Ubuntu 22.04 cloud image downloaded"
else
    echo "✓ Ubuntu 22.04 cloud image already exists"
fi

echo ""
echo "✅ All cloud images ready!"
ls -lh *.img

