#!/bin/bash
# Download Pop!_OS 24.04 Alpha with COSMIC

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ISO_DIR="${SCRIPT_DIR}/../isos"
ISO_NAME="pop-os_24.04_amd64_alpha_cosmic.iso"
ISO_PATH="${ISO_DIR}/${ISO_NAME}"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Downloading Pop!_OS 24.04 Alpha with COSMIC                        ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

mkdir -p "${ISO_DIR}"

# Pop!_OS 24.04 Alpha with COSMIC (check System76 website for latest link)
echo "📥 Checking for Pop!_OS 24.04 COSMIC Alpha download..."
echo ""
echo "Pop!_OS 24.04 with COSMIC is in alpha/beta."
echo "Download links:"
echo "  • https://pop.system76.com/"
echo "  • https://github.com/pop-os/cosmic-epoch"
echo ""
echo "Latest alpha/beta builds:"
echo "  • Intel/AMD: https://iso.pop-os.org/24.04/amd64/intel/... (check website)"
echo "  • NVIDIA: https://iso.pop-os.org/24.04/amd64/nvidia/... (check website)"
echo ""

# Alternative: Use direct download if available
DOWNLOAD_URL="https://iso.pop-os.org/24.04/amd64/intel/11/pop-os_24.04_amd64_intel_11.iso"

echo "🔍 Attempting to download from: ${DOWNLOAD_URL}"
echo ""

if wget --spider "${DOWNLOAD_URL}" 2>/dev/null; then
    echo "✅ Found download link"
    echo "📥 Downloading Pop!_OS 24.04 with COSMIC (~3GB)..."
    echo "   This may take 5-15 minutes depending on your connection..."
    echo ""
    
    wget -O "${ISO_PATH}" "${DOWNLOAD_URL}"
    
    echo ""
    echo "✅ Downloaded: ${ISO_PATH}"
    echo "   Size: $(du -h ${ISO_PATH} | cut -f1)"
else
    echo "❌ Automated download not available"
    echo ""
    echo "📝 Manual download steps:"
    echo "   1. Visit: https://pop.system76.com/"
    echo "   2. Download Pop!_OS 24.04 Alpha (with COSMIC)"
    echo "   3. Save to: ${ISO_PATH}"
    echo ""
    echo "Or check the latest alpha builds at:"
    echo "   https://github.com/pop-os/cosmic-epoch/releases"
    echo ""
    exit 1
fi

# Copy to libvirt directory
echo "📋 Copying to libvirt directory..."
sudo cp "${ISO_PATH}" /var/lib/libvirt/images/pop-os-24.04-cosmic.iso

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  ✅ Pop!_OS 24.04 COSMIC Ready                                       ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 ISO Location:"
echo "   agentReagents: ${ISO_PATH}"
echo "   libvirt:       /var/lib/libvirt/images/pop-os-24.04-cosmic.iso"
echo ""
echo "🚀 Next: Run the template builder"
echo "   ./scripts/build-popos-from-iso.sh"
echo ""

