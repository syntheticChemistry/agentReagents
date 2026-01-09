#!/bin/bash
# Build Ubuntu 24.04 Baseline Template (No RustDesk)
# Purpose: Create clean Ubuntu 24.04 desktop VM for baseline testing

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Configuration
BASE_IMAGE="${REAGENTS_ROOT}/images/cloud/ubuntu-24.04-server-cloudimg-amd64.img"
VM_NAME="ubuntu-24-baseline-builder"
WORK_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"
FINAL_TEMPLATE="/var/lib/libvirt/images/ubuntu-24.04-baseline-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Building Ubuntu 24.04 Baseline Template                            ║"
echo "║  (GNOME Desktop + Wayland, NO RustDesk)                             ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Verify base image exists
if [ ! -f "${BASE_IMAGE}" ]; then
    echo "❌ Base image not found: ${BASE_IMAGE}"
    echo "Run: ./scripts/download-cloud-images.sh"
    exit 1
fi

echo "✅ Using cloud image: $(basename ${BASE_IMAGE})"
echo ""

# Clean up any existing builder VM
echo "🧹 Cleaning up any existing builder VMs..."
sudo virsh destroy "${VM_NAME}" 2>/dev/null || true
sudo virsh undefine "${VM_NAME}" 2>/dev/null || true
sudo rm -f "${WORK_DISK}"

# Copy base image to libvirt directory
echo "📋 Preparing base image..."
sudo cp "${BASE_IMAGE}" /var/lib/libvirt/images/ubuntu-24-base.qcow2
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/ubuntu-24-base.qcow2

# Create working disk from base (25GB for desktop environment)
echo "💿 Creating 25GB working disk..."
sudo qemu-img create -f qcow2 -b /var/lib/libvirt/images/ubuntu-24-base.qcow2 -F qcow2 "${WORK_DISK}" 25G
sudo qemu-img resize "${WORK_DISK}" 25G

# Generate SSH key for access
TEMP_DIR=$(mktemp -d)
SSH_KEY="${TEMP_DIR}/id_rsa"
echo "🔑 Generating SSH key..."
ssh-keygen -t rsa -b 2048 -f "${SSH_KEY}" -N "" -C "ubuntu-baseline-builder" >/dev/null 2>&1
SSH_PUB_KEY=$(cat "${SSH_KEY}.pub")

# Create cloud-init user-data
USER_DATA="${TEMP_DIR}/user-data"
cat > "${USER_DATA}" << EOF
#cloud-config
users:
  - name: ubuntu
    groups: users, admin, sudo
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
      - ${SSH_PUB_KEY}

chpasswd:
  list: |
    ubuntu:ubuntu
  expire: false

package_update: true
package_upgrade: true

packages:
  # Desktop environment
  - ubuntu-desktop-minimal
  - gnome-shell
  - gdm3
  # Development tools
  - build-essential
  - git
  - curl
  - wget
  - vim
  # Wayland essentials
  - libwayland-client0
  - libwayland-server0
  - xwayland
  # Remote access helpers
  - openssh-server
  - avahi-daemon
  # System utilities
  - net-tools
  - dbus-x11

runcmd:
  # Enable Wayland for GDM
  - sed -i 's/#WaylandEnable=false/WaylandEnable=true/' /etc/gdm3/custom.conf
  
  # Enable auto-login for testing (can be disabled later)
  - |
    cat >> /etc/gdm3/custom.conf << 'GDMCONF'
    [daemon]
    AutomaticLoginEnable=true
    AutomaticLogin=ubuntu
    GDMCONF
  
  # Enable GDM
  - systemctl enable gdm3
  - systemctl set-default graphical.target
  
  # Ensure SSH is enabled
  - systemctl enable ssh
  - systemctl start ssh
  
  # Clean up
  - apt-get autoremove -y
  - apt-get clean
  - sync

power_state:
  mode: poweroff
  timeout: 600
  condition: true
EOF

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
sudo cp "${CLOUD_INIT_ISO}" /var/lib/libvirt/images/ubuntu-baseline-cloud-init.iso
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/ubuntu-baseline-cloud-init.iso

# Create and start VM
echo ""
echo "🚀 Starting VM build process..."
echo "   This will take 10-20 minutes (desktop installation)..."
echo ""

sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk "${WORK_DISK}",device=disk,bus=virtio \
    --disk /var/lib/libvirt/images/ubuntu-baseline-cloud-init.iso,device=cdrom \
    --os-variant ubuntu24.04 \
    --virt-type kvm \
    --graphics none \
    --network network=default,model=virtio \
    --import \
    --noautoconsole

# Wait for VM to complete installation and power off
echo "⏳ Waiting for VM to complete installation..."
for i in {1..60}; do
    sleep 30
    if ! sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; then
        echo "✅ VM has powered off (installation complete)"
        break
    fi
    echo "   Still installing... ($((i*30)) seconds elapsed)"
    if [ $i -eq 60 ]; then
        echo "❌ Timeout waiting for VM to complete"
        sudo virsh destroy "${VM_NAME}" 2>/dev/null || true
        exit 1
    fi
done

# Clean up the VM definition (but keep the disk)
echo "🧹 Cleaning up builder VM..."
sudo virsh undefine "${VM_NAME}" 2>/dev/null || true
sudo rm -f /var/lib/libvirt/images/ubuntu-baseline-cloud-init.iso

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
echo "║  ✅ Ubuntu 24.04 Baseline Template Created Successfully!            ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 Template Location: ${FINAL_TEMPLATE}"
echo "📊 Template Size: $(du -h ${FINAL_TEMPLATE} | cut -f1)"
echo ""
echo "🎯 Features:"
echo "   • Ubuntu 24.04 LTS"
echo "   • GNOME Desktop (minimal)"
echo "   • Wayland compositor"
echo "   • SSH enabled"
echo "   • No RustDesk (baseline)"
echo ""
echo "👤 Default Credentials:"
echo "   Username: ubuntu"
echo "   Password: ubuntu"
echo ""
echo "🚀 Ready to use with benchScale!"
echo ""

