#!/usr/bin/env bash
# Download Pop!_OS 24.04 LTS ISO with COSMIC
# Pop!_OS 24.04 includes COSMIC desktop built-in

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ISO_DIR="${SCRIPT_DIR}/../images/iso"
mkdir -p "${ISO_DIR}"

echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║  Downloading Pop!_OS 24.04 LTS (with COSMIC)                            ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""

# Pop!_OS 24.04 LTS with COSMIC
# Direct download from Pop!_OS official site
POPOS_VERSION="24.04"
POPOS_ISO="pop-os_${POPOS_VERSION}_amd64_intel.iso"
POPOS_URL="https://pop-iso.sfo2.cdn.digitaloceanspaces.com/24.04/amd64/intel/pop-os_${POPOS_VERSION}_amd64_intel.iso"

# Alternative mirrors if main fails
MIRROR1="https://iso.pop-os.org/24.04/amd64/intel/pop-os_${POPOS_VERSION}_amd64_intel.iso"
MIRROR2="http://iso.pop-os.org/24.04/amd64/intel/pop-os_${POPOS_VERSION}_amd64_intel.iso"

cd "${ISO_DIR}"

# Download ISO
if [ ! -f "${POPOS_ISO}" ]; then
    echo "Downloading Pop!_OS 24.04 ISO..."
    echo "Size: ~3.2GB (this will take a few minutes)"
    echo ""
    
    # Try main URL first
    echo "Trying: ${POPOS_URL}"
    if wget --progress=bar:force --timeout=30 --tries=2 "${POPOS_URL}" -O "${POPOS_ISO}.tmp" 2>&1; then
        echo "✅ Downloaded from main URL"
        mv "${POPOS_ISO}.tmp" "${POPOS_ISO}"
    else
        echo "⚠️  Main URL failed, trying mirror 1..."
        if wget --progress=bar:force --timeout=30 --tries=2 "${MIRROR1}" -O "${POPOS_ISO}.tmp" 2>&1; then
            echo "✅ Downloaded from mirror 1"
            mv "${POPOS_ISO}.tmp" "${POPOS_ISO}"
        else
            echo "⚠️  Mirror 1 failed, trying mirror 2..."
            if wget --progress=bar:force --timeout=30 --tries=2 "${MIRROR2}" -O "${POPOS_ISO}.tmp" 2>&1; then
                echo "✅ Downloaded from mirror 2"
                mv "${POPOS_ISO}.tmp" "${POPOS_ISO}"
            else
                echo "❌ All download attempts failed"
                echo ""
                echo "Please download manually from:"
                echo "  https://pop.system76.com/download"
                echo ""
                echo "Save as: ${ISO_DIR}/${POPOS_ISO}"
                rm -f "${POPOS_ISO}.tmp"
                exit 1
            fi
        fi
    fi
    
    echo ""
    echo "✅ Downloaded: ${POPOS_ISO}"
else
    echo "✅ ISO already exists: ${POPOS_ISO}"
fi

# Note: Checksum verification skipped (can verify manually if needed)
echo ""
echo "ℹ️  Verify ISO integrity at: https://pop.system76.com/download"

# Show details
ISO_SIZE=$(du -h "${POPOS_ISO}" | cut -f1)
echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║  Download Complete                                                       ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "Pop!_OS 24.04 LTS (with COSMIC)"
echo "  File: ${ISO_DIR}/${POPOS_ISO}"
echo "  Size: ${ISO_SIZE}"
echo ""
echo "Next steps:"
echo "  cd ../benchScale"
echo "  ./create-popos-cosmic-rustdesk.sh"
echo ""

