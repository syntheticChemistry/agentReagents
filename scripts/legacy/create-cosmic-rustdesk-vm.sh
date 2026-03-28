#!/bin/bash
# Quick script to create a VM from Pop!_OS 24 + COSMIC + RustDesk template

set -e

VM_NAME="cosmic-rustdesk-test-$(date +%Y%m%d-%H%M%S)"
TEMPLATE="/var/lib/libvirt/images/popos-24-cosmic-rustdesk-template.qcow2"
VM_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║  Creating Pop!_OS 24 + COSMIC + RustDesk VM                             ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""

# Verify template exists
if [ ! -f "${TEMPLATE}" ]; then
    echo "❌ Template not found: ${TEMPLATE}"
    echo "Run: cd agentReagents/scripts && sudo ./build-popos-24-cosmic-rustdesk.sh"
    exit 1
fi

echo "✅ Template found: ${TEMPLATE}"
echo "📦 Creating VM: ${VM_NAME}"
echo ""

# Create COW disk from template
echo "💿 Creating disk from template..."
sudo qemu-img create -f qcow2 -b "${TEMPLATE}" -F qcow2 "${VM_DISK}" 35G

# Create VM
echo "🚀 Creating VM..."
sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk "${VM_DISK}",format=qcow2 \
    --os-variant ubuntu24.04 \
    --network network=default \
    --graphics vnc,listen=0.0.0.0 \
    --import \
    --noautoconsole

echo ""
echo "✅ VM Created Successfully!"
echo ""

# Wait a moment for VM to start
sleep 5

# Get IP address
echo "🔍 Getting VM information..."
IP=$(sudo virsh domifaddr "${VM_NAME}" | grep ipv4 | awk '{print $4}' | cut -d/ -f1)
VNC=$(sudo virsh vncdisplay "${VM_NAME}")

echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║  VM Ready!                                                               ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "VM Name:  ${VM_NAME}"
echo "IP:       ${IP:-Waiting for IP...}"
echo "VNC:      localhost${VNC}"
echo ""
echo "🎯 Access Methods:"
echo ""
echo "1. VNC (for GUI access):"
echo "   vncviewer localhost${VNC}"
echo ""
echo "2. SSH:"
echo "   ssh cosmic@${IP:-<wait for IP>}"
echo "   Password: CosmicDesk2025!"
echo ""
echo "3. RustDesk:"
echo "   • Access via VNC first"
echo "   • Login to COSMIC desktop (auto-login should work)"
echo "   • RustDesk will auto-start"
echo "   • Note the ID and password shown"
echo "   • Use that ID to connect from remote computer"
echo ""
echo "📝 Quick Commands:"
echo ""
echo "# List all VMs with VNC ports"
echo "cd /home/flockgate/Developemt/syntheticChemistry && ./list-vms.sh"
echo ""
echo "# Get IP"
echo "sudo virsh domifaddr ${VM_NAME}"
echo ""
echo "# Get VNC display"
echo "sudo virsh vncdisplay ${VM_NAME}"
echo ""
echo "# Destroy VM"
echo "sudo virsh destroy ${VM_NAME}"
echo "sudo virsh undefine ${VM_NAME} --remove-all-storage"
echo ""
echo "💡 Tip: Each VM gets its own VNC port automatically:"
echo "   First VM:  :0 (port 5900)"
echo "   Second VM: :1 (port 5901)"
echo "   Third VM:  :2 (port 5902), etc."
echo ""

