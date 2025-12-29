#!/bin/bash
# Build Pop!_OS 24.04 + COSMIC + RustDesk Template

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ISO_PATH="/var/lib/libvirt/images/pop-os_24.04_amd64_nvidia_22.iso"
RUSTDESK_DEB="${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb"
VM_NAME="popos24-cosmic-template-builder"
DISK_PATH="/var/lib/libvirt/images/${VM_NAME}.qcow2"
FINAL_TEMPLATE="/var/lib/libvirt/images/popos-24.04-cosmic-rustdesk-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Building Pop!_OS 24.04 + COSMIC + RustDesk Template                ║"
echo "║  Primary Target for ionChannel Validation                           ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Copy ISO to libvirt location if not there
if [ ! -f "${ISO_PATH}" ]; then
    echo "📋 Copying Pop!_OS 24.04 ISO to libvirt directory..."
    sudo cp "${REAGENTS_ROOT}/isos/pop-os_24.04_amd64_nvidia_22.iso" "${ISO_PATH}"
    echo "✅ ISO copied"
fi

# Verify files exist
if [ ! -f "${ISO_PATH}" ]; then
    echo "❌ Pop!_OS 24.04 ISO not found"
    exit 1
fi
echo "✅ Found Pop!_OS 24.04 ISO (3.4GB)"

if [ ! -f "${RUSTDESK_DEB}" ]; then
    echo "❌ RustDesk .deb not found at: ${RUSTDESK_DEB}"
    exit 1
fi
echo "✅ Found RustDesk .deb"

# Check if template already exists
if [ -f "${FINAL_TEMPLATE}" ]; then
    echo ""
    echo "⚠️  Template already exists: ${FINAL_TEMPLATE}"
    echo "   Size: $(du -h ${FINAL_TEMPLATE} | cut -f1)"
    echo ""
    read -p "Do you want to rebuild it? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "✅ Using existing template"
        exit 0
    fi
    # Clean up existing
    sudo virsh destroy "${VM_NAME}" 2>/dev/null || true
    sudo virsh undefine "${VM_NAME}" 2>/dev/null || true
    sudo rm -f "${DISK_PATH}"
fi

echo ""
echo "🔧 Installation Steps:"
echo "   1. VM will open in virt-viewer"
echo "   2. Install Pop!_OS 24.04:"
echo "      • Language: English"
echo "      • Keyboard: US"
echo "      • Installation: Clean Install"
echo "      • User: iontest"
echo "      • Password: iontest123"
echo "      • Hostname: popos24-cosmic"
echo "   3. ⭐ IMPORTANT: Select COSMIC desktop (not GNOME!)"
echo "   4. Wait for installation (~15-20 minutes)"
echo "   5. Reboot and login to COSMIC desktop"
echo ""
echo "After installation, run:"
echo "   ${SCRIPT_DIR}/configure-popos-24-template.sh"
echo ""
read -p "Press Enter to create VM and start installation..."

# Create disk
echo ""
echo "💿 Creating 30GB disk..."
sudo qemu-img create -f qcow2 "${DISK_PATH}" 30G

# Create VM with ISO attached
echo "🚀 Creating VM..."
sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk path="${DISK_PATH}",format=qcow2 \
    --cdrom "${ISO_PATH}" \
    --os-variant ubuntu24.04 \
    --network network=default \
    --graphics spice \
    --noautoconsole

echo ""
echo "✅ VM created: ${VM_NAME}"
echo ""
echo "📺 Opening virt-viewer..."
sleep 2

# Open virt-viewer
virt-viewer "${VM_NAME}" &

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Installation Started                                                ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📝 Installation Checklist:"
echo "   [ ] Select Clean Install"
echo "   [ ] Create user: iontest / iontest123"
echo "   [ ] ⭐ Choose COSMIC desktop (NOT GNOME)"
echo "   [ ] Wait for installation to complete"
echo "   [ ] Reboot"
echo "   [ ] Login to COSMIC"
echo ""
echo "⏳ After logging into COSMIC desktop, run:"
echo "   cd agentReagents"
echo "   ./scripts/configure-popos-24-template.sh"
echo ""
echo "🎯 The VM is now installing Pop!_OS 24.04 with COSMIC!"

