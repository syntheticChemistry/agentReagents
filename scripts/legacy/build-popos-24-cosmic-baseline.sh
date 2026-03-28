#!/bin/bash
# Build Pop!_OS 24 COSMIC Baseline Template (No RustDesk)
# Purpose: Create clean Pop!_OS 24 with COSMIC desktop for baseline testing

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Configuration
BASE_IMAGE="${REAGENTS_ROOT}/images/cloud/ubuntu-24.04-server-cloudimg-amd64.img"
VM_NAME="popos-cosmic-baseline-builder"
WORK_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"
FINAL_TEMPLATE="/var/lib/libvirt/images/popos-24-cosmic-baseline-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Building Pop!_OS 24 COSMIC Baseline Template                       ║"
echo "║  (COSMIC Desktop + Wayland, NO RustDesk)                            ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Verify base image exists
if [ ! -f "${BASE_IMAGE}" ]; then
    echo "❌ Base image not found: ${BASE_IMAGE}"
    echo "Run: ./scripts/download-cloud-images.sh"
    exit 1
fi

echo "✅ Using Ubuntu 24.04 cloud image as base (will add COSMIC)"
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

# Create working disk from base (30GB for COSMIC desktop)
echo "💿 Creating 30GB working disk..."
sudo qemu-img create -f qcow2 -b /var/lib/libvirt/images/ubuntu-24-base.qcow2 -F qcow2 "${WORK_DISK}" 30G
sudo qemu-img resize "${WORK_DISK}" 30G

# Generate SSH key for access
TEMP_DIR=$(mktemp -d)
SSH_KEY="${TEMP_DIR}/id_rsa"
echo "🔑 Generating SSH key..."
ssh-keygen -t rsa -b 2048 -f "${SSH_KEY}" -N "" -C "popos-cosmic-baseline-builder" >/dev/null 2>&1
SSH_PUB_KEY=$(cat "${SSH_KEY}.pub")

# Create cloud-init user-data
USER_DATA="${TEMP_DIR}/user-data"
cat > "${USER_DATA}" << EOF
#cloud-config
users:
  - name: cosmic
    groups: users, admin, sudo
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
      - ${SSH_PUB_KEY}

chpasswd:
  list: |
    cosmic:cosmic
  expire: false

package_update: true
package_upgrade: true

packages:
  # Base system
  - build-essential
  - git
  - curl
  - wget
  - vim
  # Wayland essentials
  - libwayland-client0
  - libwayland-server0
  - xwayland
  # Required for COSMIC
  - software-properties-common
  - gnupg2
  - ca-certificates
  # Remote access
  - openssh-server
  - avahi-daemon
  # System utilities
  - net-tools
  - dbus-x11
  - pipewire
  - wireplumber

runcmd:
  # Add System76 COSMIC repository
  - |
    echo "Adding System76 COSMIC repository..."
    curl -fsSL https://apt.system76.com/signing-key.asc | gpg --dearmor -o /etc/apt/keyrings/system76.gpg
    echo "deb [signed-by=/etc/apt/keyrings/system76.gpg] https://apt.system76.com/cosmic noble main" | tee /etc/apt/sources.list.d/system76-cosmic.list
    apt-get update
  
  # Install COSMIC Desktop
  - |
    echo "Installing COSMIC Desktop..."
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
      cosmic-session \
      cosmic-greeter \
      cosmic-comp \
      cosmic-panel \
      cosmic-launcher \
      cosmic-applets \
      cosmic-settings \
      cosmic-files \
      cosmic-term \
      cosmic-edit
  
  # Enable COSMIC greeter
  - systemctl enable cosmic-greeter
  - systemctl set-default graphical.target
  
  # Enable auto-login for testing
  - mkdir -p /etc/cosmic-greeter
  - |
    cat > /etc/cosmic-greeter/auto-login.conf << 'AUTOCONF'
    [daemon]
    AutomaticLoginEnable=true
    AutomaticLogin=cosmic
    AUTOCONF
  
  # Ensure SSH is enabled
  - systemctl enable ssh
  - systemctl start ssh
  
  # Enable PipeWire for screen capture
  - systemctl --user enable pipewire
  - systemctl --user enable wireplumber
  
  # Clean up
  - apt-get autoremove -y
  - apt-get clean
  - sync

power_state:
  mode: poweroff
  timeout: 900
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
sudo cp "${CLOUD_INIT_ISO}" /var/lib/libvirt/images/popos-cosmic-baseline-cloud-init.iso
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/popos-cosmic-baseline-cloud-init.iso

# Create and start VM
echo ""
echo "🚀 Starting VM build process..."
echo "   This will take 15-30 minutes (COSMIC installation)..."
echo ""

sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk "${WORK_DISK}",device=disk,bus=virtio \
    --disk /var/lib/libvirt/images/popos-cosmic-baseline-cloud-init.iso,device=cdrom \
    --os-variant ubuntu24.04 \
    --virt-type kvm \
    --graphics none \
    --network network=default,model=virtio \
    --import \
    --noautoconsole

# Wait for VM to complete installation and power off
echo "⏳ Waiting for VM to complete installation..."
for i in {1..90}; do
    sleep 30
    if ! sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; then
        echo "✅ VM has powered off (installation complete)"
        break
    fi
    echo "   Still installing... ($((i*30)) seconds elapsed)"
    if [ $i -eq 90 ]; then
        echo "❌ Timeout waiting for VM to complete"
        sudo virsh destroy "${VM_NAME}" 2>/dev/null || true
        exit 1
    fi
done

# Clean up the VM definition (but keep the disk)
echo "🧹 Cleaning up builder VM..."
sudo virsh undefine "${VM_NAME}" 2>/dev/null || true
sudo rm -f /var/lib/libvirt/images/popos-cosmic-baseline-cloud-init.iso

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
echo "║  ✅ Pop!_OS 24 COSMIC Baseline Template Created Successfully!       ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 Template Location: ${FINAL_TEMPLATE}"
echo "📊 Template Size: $(du -h ${FINAL_TEMPLATE} | cut -f1)"
echo ""
echo "🎯 Features:"
echo "   • Pop!_OS 24 (Ubuntu 24.04 base)"
echo "   • COSMIC Desktop (System76)"
echo "   • Wayland compositor (cosmic-comp)"
echo "   • PipeWire for screen capture"
echo "   • SSH enabled"
echo "   • No RustDesk (baseline)"
echo ""
echo "👤 Default Credentials:"
echo "   Username: cosmic"
echo "   Password: cosmic"
echo ""
echo "🚀 Ready to use with benchScale!"
echo ""

