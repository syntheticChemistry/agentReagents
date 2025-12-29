#!/bin/bash
# Finalize Pop!_OS Template - Save as template image

set -e

VM_NAME="popos-cosmic-template-builder"
DISK_PATH="/var/lib/libvirt/images/${VM_NAME}.qcow2"
FINAL_TEMPLATE="/var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_TEMPLATE="${SCRIPT_DIR}/../images/templates/popos-cosmic-rustdesk-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Finalizing Pop!_OS + COSMIC + RustDesk Template                    ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Verify VM is shut down
if sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; then
    echo "❌ VM is still running. Please shut it down first:"
    echo "   sudo virsh shutdown ${VM_NAME}"
    echo "   # Or from inside VM: sudo shutdown -h now"
    exit 1
fi

echo "✅ VM is shut down"
echo ""

# Compress and optimize
echo "🗜️  Compressing and optimizing image..."
sudo qemu-img convert -O qcow2 -c "${DISK_PATH}" "${FINAL_TEMPLATE}.tmp"
sudo mv "${FINAL_TEMPLATE}.tmp" "${FINAL_TEMPLATE}"

# Set permissions
echo "🔐 Setting permissions..."
sudo chown libvirt-qemu:kvm "${FINAL_TEMPLATE}"
sudo chmod 644 "${FINAL_TEMPLATE}"

# Copy to agentReagents
echo "📦 Copying to agentReagents..."
mkdir -p "$(dirname "${REAGENTS_TEMPLATE}")"
sudo cp "${FINAL_TEMPLATE}" "${REAGENTS_TEMPLATE}"
sudo chown $(whoami):$(whoami) "${REAGENTS_TEMPLATE}"

# Save intermediate
INTERMEDIATE="${SCRIPT_DIR}/../images/intermediates/popos-cosmic-$(date +%Y%m%d-%H%M%S).qcow2"
echo "💾 Saving intermediate snapshot..."
mkdir -p "$(dirname "${INTERMEDIATE}")"
cp "${REAGENTS_TEMPLATE}" "${INTERMEDIATE}"

# Clean up builder VM
echo "🧹 Cleaning up builder VM..."
sudo virsh undefine "${VM_NAME}"

# Get sizes
TEMPLATE_SIZE=$(du -h "${FINAL_TEMPLATE}" | cut -f1)
DISK_SIZE=$(du -h "${DISK_PATH}" | cut -f1)

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  ✅ Pop!_OS + COSMIC + RustDesk Template Ready!                     ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 Template Locations:"
echo "   libvirt:       ${FINAL_TEMPLATE}"
echo "   agentReagents: ${REAGENTS_TEMPLATE}"
echo "   Size:          ${TEMPLATE_SIZE}"
echo ""
echo "📦 Contents:"
echo "   • Pop!_OS 22.04 with NVIDIA drivers"
echo "   • COSMIC desktop (Wayland)"
echo "   • RustDesk 1.2.3 (auto-start enabled)"
echo "   • User: iontest / iontest123"
echo ""
echo "🚀 Ready to use!"
echo ""
echo "Test with:"
echo "   cd ../../ionChannel"
echo "   cargo run --bin autonomous-rustdesk-benchscale --features benchscale"
echo ""
echo "Or A/B validation:"
echo "   cargo run --bin ab-validation --features benchscale"
echo ""

# Optional: Clean up original disk
read -p "Remove original builder disk (${DISK_SIZE})? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo rm "${DISK_PATH}"
    echo "✅ Removed builder disk"
fi

echo "✅ Template creation complete!"

