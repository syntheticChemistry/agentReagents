#!/bin/bash
# Download cloud images for automated VM builds

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CLOUD_DIR="${REAGENTS_ROOT}/images/cloud"

mkdir -p "${CLOUD_DIR}"
cd "${CLOUD_DIR}"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Downloading Cloud Images (~2GB)                                     ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Ubuntu 24.04 Server Cloud Image
if [ ! -f "ubuntu-24.04-server-cloudimg-amd64.img" ]; then
    echo "📥 Downloading Ubuntu 24.04 Server Cloud Image (~700MB)..."
    wget -c --progress=bar:force:noscroll \
        -O ubuntu-24.04-server-cloudimg-amd64.img \
        https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
    echo "✅ Ubuntu 24.04 cloud image downloaded"
else
    echo "✓ Ubuntu 24.04 cloud image already exists"
fi

# Ubuntu 22.04 Server Cloud Image
if [ ! -f "ubuntu-22.04-server-cloudimg-amd64.img" ]; then
    echo "📥 Downloading Ubuntu 22.04 Server Cloud Image (~600MB)..."
    wget -c --progress=bar:force:noscroll \
        -O ubuntu-22.04-server-cloudimg-amd64.img \
        https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img
    echo "✅ Ubuntu 22.04 cloud image downloaded"
else
    echo "✓ Ubuntu 22.04 cloud image already exists"
fi

echo ""
echo "✅ All cloud images ready!"
ls -lh *.img

