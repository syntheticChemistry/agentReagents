#!/bin/bash
# Quick ISO download script for agentReagents
# Run from agentReagents/ directory

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ISO_DIR="${REAGENTS_ROOT}/isos"

mkdir -p "${ISO_DIR}"
cd "${ISO_DIR}"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Downloading agentReagents ISOs                                      ║"
echo "║  Total: ~13GB (3 ISOs)                                               ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Pop!_OS 22.04
if [ ! -f "pop-os_22.04_amd64_nvidia_22.iso" ]; then
    echo "📥 Downloading Pop!_OS 22.04 LTS (~3.4GB)..."
    wget -c --progress=bar:force:noscroll \
        https://iso.pop-os.org/22.04/amd64/nvidia/22/pop-os_22.04_amd64_nvidia_22.iso
    echo "✅ Pop!_OS 22.04 downloaded"
else
    echo "✓ Pop!_OS 22.04 already exists"
fi

# Pop!_OS 24.04
if [ ! -f "pop-os_24.04_amd64_nvidia_22.iso" ]; then
    echo "📥 Downloading Pop!_OS 24.04 LTS (~3.4GB)..."
    wget -c --progress=bar:force:noscroll \
        https://iso.pop-os.org/24.04/amd64/nvidia/22/pop-os_24.04_amd64_nvidia_22.iso
    echo "✅ Pop!_OS 24.04 downloaded"
else
    echo "✓ Pop!_OS 24.04 already exists"
fi

# Ubuntu 24.04
if [ ! -f "ubuntu-24.04.3-desktop-amd64.iso" ]; then
    echo "📥 Downloading Ubuntu 24.04 LTS (~6.0GB)..."
    wget -c --progress=bar:force:noscroll \
        https://releases.ubuntu.com/noble/ubuntu-24.04.3-desktop-amd64.iso
    echo "✅ Ubuntu 24.04 downloaded"
else
    echo "✓ Ubuntu 24.04 already exists"
fi

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  ✅ All ISOs Ready!                                                   ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
ls -lh *.iso

