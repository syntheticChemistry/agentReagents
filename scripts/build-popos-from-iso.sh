#!/bin/bash
# Build Pop!_OS + COSMIC + RustDesk Template from ISO
# Uses the existing Pop!_OS ISO in agentReagents

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ISO_PATH="/var/lib/libvirt/images/pop-os_22.04_amd64_nvidia_22.iso"
RUSTDESK_DEB="${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb"
TEMPLATE_DIR="${REAGENTS_ROOT}/images/templates"
VM_NAME="popos-cosmic-template-builder"
DISK_PATH="/var/lib/libvirt/images/${VM_NAME}.qcow2"
FINAL_TEMPLATE="/var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Building Pop!_OS + COSMIC + RustDesk Template                      ║"
echo "║  From ISO: pop-os_22.04_amd64_nvidia_22.iso                         ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Verify ISO exists
if [ ! -f "${ISO_PATH}" ]; then
    echo "❌ Pop!_OS ISO not found at: ${ISO_PATH}"
    exit 1
fi
echo "✅ Found Pop!_OS ISO (3GB)"

# Verify RustDesk deb exists
if [ ! -f "${RUSTDESK_DEB}" ]; then
    echo "❌ RustDesk .deb not found at: ${RUSTDESK_DEB}"
    exit 1
fi
echo "✅ Found RustDesk .deb"

# Create template directory
mkdir -p "${TEMPLATE_DIR}"

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
        # Copy to agentReagents if not there
        if [ ! -f "${TEMPLATE_DIR}/popos-cosmic-rustdesk-template.qcow2" ]; then
            cp "${FINAL_TEMPLATE}" "${TEMPLATE_DIR}/popos-cosmic-rustdesk-template.qcow2"
            echo "✅ Copied to agentReagents"
        fi
        exit 0
    fi
    # Remove existing
    sudo virsh destroy "${VM_NAME}" 2>/dev/null || true
    sudo virsh undefine "${VM_NAME}" 2>/dev/null || true
    sudo rm -f "${DISK_PATH}"
fi

echo ""
echo "🔧 This script will:"
echo "   1. Create a new VM from the Pop!_OS ISO"
echo "   2. You'll need to complete the Pop!_OS installation manually"
echo "   3. After installation, we'll configure and save as template"
echo ""
echo "📋 Manual steps you'll need to do:"
echo "   • Select language and keyboard"
echo "   • Choose 'Clean Install'"
echo "   • Create user: iontest / password: iontest123"
echo "   • Wait for installation (15-20 minutes)"
echo "   • Reboot and login"
echo ""
read -p "Press Enter to create the VM and open virt-viewer..."

# Create disk
echo "💿 Creating 25GB disk..."
sudo qemu-img create -f qcow2 "${DISK_PATH}" 25G

# Create VM with ISO attached
echo "🚀 Creating VM..."
sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk path="${DISK_PATH}",format=qcow2 \
    --cdrom "${ISO_PATH}" \
    --os-variant ubuntu22.04 \
    --network network=default \
    --graphics spice \
    --noautoconsole

echo ""
echo "✅ VM created: ${VM_NAME}"
echo ""
echo "📺 Opening virt-viewer..."
echo "   Complete the Pop!_OS installation with these details:"
echo "   • Username: iontest"
echo "   • Password: iontest123"
echo "   • Hostname: popos-template"
echo ""
echo "⏳ When installation is complete and you've rebooted and logged in,"
echo "   run the post-install script:"
echo "   ${SCRIPT_DIR}/configure-popos-template.sh"
echo ""

# Open virt-viewer
virt-viewer "${VM_NAME}" &

echo "🎯 VM is installing. Come back when logged into the desktop!"

