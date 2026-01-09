#!/bin/bash
# Build Ubuntu 24.04 + RustDesk Template
# Purpose: Add RustDesk to Ubuntu 24.04 baseline for testing Wayland remote desktop issue

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Configuration
BASELINE_TEMPLATE="/var/lib/libvirt/images/ubuntu-24.04-baseline-template.qcow2"
RUSTDESK_DEB="${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb"
VM_NAME="ubuntu-24-rustdesk-builder"
WORK_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"
FINAL_TEMPLATE="/var/lib/libvirt/images/ubuntu-24.04-rustdesk-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Building Ubuntu 24.04 + RustDesk Template                          ║"
echo "║  (From baseline template + RustDesk installation)                   ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Verify baseline template exists
if [ ! -f "${BASELINE_TEMPLATE}" ]; then
    echo "❌ Baseline template not found: ${BASELINE_TEMPLATE}"
    echo "Run: ./scripts/build-ubuntu-24-baseline.sh"
    exit 1
fi

# Verify RustDesk .deb exists
if [ ! -f "${RUSTDESK_DEB}" ]; then
    echo "❌ RustDesk .deb not found: ${RUSTDESK_DEB}"
    echo "Run: ./scripts/download-rustdesk.sh"
    exit 1
fi

echo "✅ Using baseline template: $(basename ${BASELINE_TEMPLATE})"
echo "✅ RustDesk package: $(basename ${RUSTDESK_DEB})"
echo ""

# Clean up any existing builder VM
echo "🧹 Cleaning up any existing builder VMs..."
sudo virsh destroy "${VM_NAME}" 2>/dev/null || true
sudo virsh undefine "${VM_NAME}" 2>/dev/null || true
sudo rm -f "${WORK_DISK}"

# Create working disk from baseline template
echo "💿 Creating working disk from baseline template..."
sudo qemu-img create -f qcow2 -b "${BASELINE_TEMPLATE}" -F qcow2 "${WORK_DISK}" 30G
sudo qemu-img resize "${WORK_DISK}" 30G

# Copy RustDesk to accessible location
echo "📦 Preparing RustDesk package..."
sudo cp "${RUSTDESK_DEB}" /var/lib/libvirt/images/rustdesk.deb
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/rustdesk.deb

# Generate SSH key for access
TEMP_DIR=$(mktemp -d)
SSH_KEY="${TEMP_DIR}/id_rsa"
echo "🔑 Generating SSH key..."
ssh-keygen -t rsa -b 2048 -f "${SSH_KEY}" -N "" -C "ubuntu-rustdesk-builder" >/dev/null 2>&1
SSH_PUB_KEY=$(cat "${SSH_KEY}.pub")

# Create cloud-init user-data for RustDesk installation
USER_DATA="${TEMP_DIR}/user-data"
cat > "${USER_DATA}" << 'EOF'
#cloud-config
users:
  - name: ubuntu
    groups: users, admin, sudo
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
      - SSH_PUB_KEY_PLACEHOLDER

chpasswd:
  list: |
    ubuntu:ubuntu
  expire: false

write_files:
  - path: /tmp/install-rustdesk.sh
    permissions: '0755'
    content: |
      #!/bin/bash
      set -e
      echo "Installing RustDesk..."
      
      # Install dependencies
      apt-get update
      DEBIAN_FRONTEND=noninteractive apt-get install -y \
        libxcb-randr0 \
        libxcb-xtest0 \
        libxcb-xfixes0 \
        libxcb-shape0 \
        libxcb-keysyms1 \
        libgstreamer1.0-0 \
        libgstreamer-plugins-base1.0-0 \
        libpulse0 \
        libxdo3
      
      # Install RustDesk
      dpkg -i /var/lib/libvirt/images/rustdesk.deb || apt-get install -f -y
      
      # Enable RustDesk service
      systemctl enable rustdesk
      systemctl start rustdesk || true
      
      echo "RustDesk installation complete!"

runcmd:
  - /tmp/install-rustdesk.sh
  - rm -f /tmp/install-rustdesk.sh
  - apt-get autoremove -y
  - apt-get clean
  - sync

power_state:
  mode: poweroff
  timeout: 600
  condition: true
EOF

# Replace SSH key placeholder
sed -i "s|SSH_PUB_KEY_PLACEHOLDER|${SSH_PUB_KEY}|" "${USER_DATA}"

# Create meta-data
META_DATA="${TEMP_DIR}/meta-data"
cat > "${META_DATA}" << EOF
instance-id: ${VM_NAME}
local-hostname: ${VM_NAME}
EOF

# Create cloud-init ISO
CLOUD_INIT_ISO="${TEMP_DIR}/cloud-init.iso"
echo "☁️  Creating cloud-init ISO..."
genisoimage -output "${CLOUD_INIT_ISO}" -volid cidata -joliet -rock "${USER_DATA}" "${META_DATA}" >/dev/null 2>&1
sudo cp "${CLOUD_INIT_ISO}" /var/lib/libvirt/images/ubuntu-rustdesk-cloud-init.iso
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/ubuntu-rustdesk-cloud-init.iso

# Create and start VM
echo ""
echo "🚀 Starting RustDesk installation process..."
echo "   This will take 5-10 minutes..."
echo ""

sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk "${WORK_DISK}",device=disk,bus=virtio \
    --disk /var/lib/libvirt/images/ubuntu-rustdesk-cloud-init.iso,device=cdrom \
    --os-variant ubuntu24.04 \
    --virt-type kvm \
    --graphics none \
    --network network=default,model=virtio \
    --import \
    --noautoconsole

# Wait for VM to complete installation and power off
echo "⏳ Waiting for VM to complete installation..."
for i in {1..40}; do
    sleep 30
    if ! sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; then
        echo "✅ VM has powered off (installation complete)"
        break
    fi
    echo "   Still installing... ($((i*30)) seconds elapsed)"
    if [ $i -eq 40 ]; then
        echo "❌ Timeout waiting for VM to complete"
        sudo virsh destroy "${VM_NAME}" 2>/dev/null || true
        exit 1
    fi
done

# Clean up the VM definition (but keep the disk)
echo "🧹 Cleaning up builder VM..."
sudo virsh undefine "${VM_NAME}" 2>/dev/null || true
sudo rm -f /var/lib/libvirt/images/ubuntu-rustdesk-cloud-init.iso
sudo rm -f /var/lib/libvirt/images/rustdesk.deb

# Optimize and finalize the template
echo "⚡ Optimizing template image..."
sudo virt-sparsify --in-place "${WORK_DISK}"

# Move to final location
echo "📦 Finalizing template..."
sudo mv "${WORK_DISK}" "${FINAL_TEMPLATE}"
sudo chown libvirt-qemu:kvm "${FINAL_TEMPLATE}"
sudo chmod 644 "${FINAL_TEMPLATE}"

# Clean up temp directory
rm -rf "${TEMP_DIR}"

# Success!
echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  ✅ Ubuntu 24.04 + RustDesk Template Created Successfully!          ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 Template Location: ${FINAL_TEMPLATE}"
echo "📊 Template Size: $(du -h ${FINAL_TEMPLATE} | cut -f1)"
echo ""
echo "🎯 Features:"
echo "   • Ubuntu 24.04 LTS"
echo "   • GNOME Desktop (Wayland)"
echo "   • RustDesk remote desktop"
echo "   • SSH enabled"
echo ""
echo "👤 Default Credentials:"
echo "   Username: ubuntu"
echo "   Password: ubuntu"
echo ""
echo "🚀 Ready for Wayland remote desktop testing!"
echo ""

