#!/bin/bash
# Finalize Pop!_OS 24.04 + COSMIC Template

set -e

VM_NAME="popos24-cosmic-template-builder"
DISK_PATH="/var/lib/libvirt/images/${VM_NAME}.qcow2"
FINAL_TEMPLATE="/var/lib/libvirt/images/popos-24.04-cosmic-rustdesk-template.qcow2"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_TEMPLATE="${SCRIPT_DIR}/../images/templates/popos-24.04-cosmic-rustdesk-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Finalizing Pop!_OS 24.04 + COSMIC + RustDesk Template              ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Verify VM is shut down
if sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; then
    echo "❌ VM is still running. Please shut it down first:"
    echo "   From inside VM: sudo shutdown -h now"
    echo "   Or: sudo virsh shutdown ${VM_NAME}"
    exit 1
fi

echo "✅ VM is shut down"
echo ""

# Compress and optimize
echo "🗜️  Compressing and optimizing image (this may take 5-10 minutes)..."
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
INTERMEDIATE="${SCRIPT_DIR}/../images/intermediates/popos-24-cosmic-$(date +%Y%m%d-%H%M%S).qcow2"
echo "💾 Saving intermediate snapshot..."
mkdir -p "$(dirname "${INTERMEDIATE}")"
cp "${REAGENTS_TEMPLATE}" "${INTERMEDIATE}"

# Clean up builder VM
echo "🧹 Cleaning up builder VM definition..."
sudo virsh undefine "${VM_NAME}"

# Get sizes
TEMPLATE_SIZE=$(du -h "${FINAL_TEMPLATE}" | cut -f1)

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  ✅ Pop!_OS 24.04 + COSMIC + RustDesk Template Complete!            ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 Template Locations:"
echo "   libvirt:       ${FINAL_TEMPLATE}"
echo "   agentReagents: ${REAGENTS_TEMPLATE}"
echo "   Intermediate:  ${INTERMEDIATE}"
echo "   Size:          ${TEMPLATE_SIZE}"
echo ""
echo "📦 Contents:"
echo "   • Pop!_OS 24.04 LTS"
echo "   • COSMIC desktop (Wayland) ⭐"
echo "   • RustDesk 1.2.3 (auto-start)"
echo "   • User: iontest / iontest123"
echo ""
echo "🚀 Ready to use!"
echo ""
echo "Test with benchScale lab:"
echo "   cd ../benchScale"
echo "   ./scripts/create-lab.sh --topology ecoprimals-tower-2node --name test"
echo ""
echo "Or run full validation:"
echo "   cd ../../springs/primalSpring && ./scripts/validate_local_lab.sh"
echo ""

# Optional: Clean up original disk
read -p "Remove original builder disk? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo rm "${DISK_PATH}"
    echo "✅ Removed builder disk"
fi

echo ""
echo "✅ Template creation complete!"
echo "You now have a Pop!_OS 24.04 + COSMIC template for ecoPrimals validation!"

