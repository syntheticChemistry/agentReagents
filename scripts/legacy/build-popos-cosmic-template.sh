#!/bin/bash
# Build Pop!_OS 22.04 + COSMIC/Wayland + RustDesk Template Image
# Legacy template — see specs/AGENTREAGENTS_EVOLUTION.md for golden path

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEMPLATE_DIR="${SCRIPT_DIR}/../images/templates"
TEMP_DIR="/tmp/popos-template-build-$$"
VM_NAME="popos-cosmic-builder"
FINAL_TEMPLATE="${TEMPLATE_DIR}/popos-cosmic-rustdesk-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Building Pop!_OS + COSMIC/Wayland + RustDesk Template              ║"
echo "║  Target: ${FINAL_TEMPLATE}                                          ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Ensure template directory exists
mkdir -p "${TEMPLATE_DIR}"
mkdir -p "${TEMP_DIR}"

# Download Pop!_OS cloud image if not present
POPOS_IMG="${SCRIPT_DIR}/../images/base/pop-os-22.04-amd64-cloud.img"
if [ ! -f "${POPOS_IMG}" ]; then
    echo "📥 Downloading Pop!_OS 22.04 cloud image..."
    mkdir -p "$(dirname "${POPOS_IMG}")"
    
    # Pop!_OS doesn't have official cloud images, so we'll use Ubuntu 22.04 base
    # and install Pop!_OS packages on top
    wget -O "${POPOS_IMG}" \
        https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img
    
    echo "✅ Downloaded base image"
else
    echo "✅ Using existing Pop!_OS base image"
fi

# Copy to libvirt images directory with sudo
echo "📋 Copying base image to libvirt directory..."
sudo cp "${POPOS_IMG}" /var/lib/libvirt/images/popos-cosmic-base.qcow2
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/popos-cosmic-base.qcow2

# Create working disk from base
WORK_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"
echo "💿 Creating working disk (20GB)..."
sudo qemu-img create -f qcow2 -b /var/lib/libvirt/images/popos-cosmic-base.qcow2 -F qcow2 "${WORK_DISK}" 20G

# Generate SSH key for template building
SSH_KEY="${TEMP_DIR}/id_rsa"
echo "🔑 Generating SSH key..."
ssh-keygen -t rsa -b 2048 -f "${SSH_KEY}" -N "" -C "popos-template-builder" >/dev/null 2>&1
SSH_PUB_KEY=$(cat "${SSH_KEY}.pub")

# Create cloud-init user-data
USER_DATA="${TEMP_DIR}/user-data"
cat > "${USER_DATA}" << EOF
#cloud-config
users:
  - name: iontest
    groups: users, admin, sudo
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
      - ${SSH_PUB_KEY}

# Set password for iontest user
chpasswd:
  list: |
    iontest:iontest123
  expire: false

package_update: true
package_upgrade: true

packages:
  - openssh-server
  - wget
  - curl
  - net-tools
  - software-properties-common

runcmd:
  # Add Pop!_OS repository
  - add-apt-repository -y ppa:system76/pop
  - apt-get update
  
  # Install Pop!_OS desktop and COSMIC
  - DEBIAN_FRONTEND=noninteractive apt-get install -y pop-desktop cosmic-session cosmic-comp
  
  # Enable GDM for display manager
  - systemctl enable gdm
  - systemctl set-default graphical.target
  
  # Configure for Wayland (COSMIC uses Wayland)
  - echo "WaylandEnable=true" >> /etc/gdm3/custom.conf
  - echo "DefaultSession=cosmic.desktop" >> /etc/gdm3/custom.conf
  
  # Download and install RustDesk
  - wget -O /tmp/rustdesk.deb https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb
  - DEBIAN_FRONTEND=noninteractive apt-get install -y --fix-broken /tmp/rustdesk.deb || true
  - apt-get install -f -y
  
  # Configure RustDesk to start with desktop
  - mkdir -p /home/iontest/.config/autostart
  - cp /usr/share/applications/rustdesk.desktop /home/iontest/.config/autostart/ || true
  - chown -R iontest:iontest /home/iontest/.config
  
  # Clean up
  - rm -f /tmp/rustdesk.deb
  - apt-get clean
  - cloud-init clean --logs

power_state:
  mode: poweroff
  message: Template build complete, shutting down
  timeout: 300
EOF

# Create minimal meta-data
META_DATA="${TEMP_DIR}/meta-data"
cat > "${META_DATA}" << EOF
instance-id: ${VM_NAME}
local-hostname: ${VM_NAME}
EOF

echo "🚀 Creating VM with Pop!_OS + COSMIC + RustDesk..."
echo "   This will take 15-30 minutes for package installation..."
echo ""

# Use virt-install with cloud-init
sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk path="${WORK_DISK}",format=qcow2 \
    --os-variant ubuntu22.04 \
    --network network=default \
    --graphics vnc,listen=0.0.0.0 \
    --noautoconsole \
    --import \
    --cloud-init user-data="${USER_DATA}",meta-data="${META_DATA}"

echo "⏳ Waiting for VM to complete installation and shutdown..."
echo "   (This includes: Pop!_OS packages, COSMIC, RustDesk, configuration)"
echo ""

# Wait for VM to shutdown (cloud-init will power it off when done)
TIMEOUT=2400  # 40 minutes timeout
ELAPSED=0
while sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; do
    sleep 10
    ELAPSED=$((ELAPSED + 10))
    if [ $ELAPSED -ge $TIMEOUT ]; then
        echo "❌ Timeout waiting for VM to complete. Check with: sudo virsh console ${VM_NAME}"
        exit 1
    fi
    if [ $((ELAPSED % 60)) -eq 0 ]; then
        echo "   ... $((ELAPSED / 60)) minutes elapsed"
    fi
done

echo "✅ VM installation completed and shut down"

# Undefine the VM (we only need the disk)
echo "🧹 Cleaning up builder VM definition..."
sudo virsh undefine "${VM_NAME}"

# Compress and optimize the image
echo "🗜️  Compressing template image..."
sudo qemu-img convert -O qcow2 -c "${WORK_DISK}" "${WORK_DISK}.compressed"
sudo mv "${WORK_DISK}.compressed" "${WORK_DISK}"

# Copy to template directory
echo "📦 Saving template to agentReagents..."
sudo cp "${WORK_DISK}" "${FINAL_TEMPLATE}"
sudo chown $(whoami):$(whoami) "${FINAL_TEMPLATE}"
sudo chmod 644 "${FINAL_TEMPLATE}"

# Also keep a copy in libvirt for quick access
echo "📋 Creating libvirt-accessible copy..."
sudo cp "${FINAL_TEMPLATE}" /var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2

# Save intermediate snapshot
INTERMEDIATE="${TEMPLATE_DIR}/popos-cosmic-intermediate-$(date +%Y%m%d-%H%M%S).qcow2"
echo "💾 Saving intermediate snapshot..."
cp "${FINAL_TEMPLATE}" "${INTERMEDIATE}"

# Cleanup
echo "🧹 Cleaning up temporary files..."
rm -rf "${TEMP_DIR}"

# Get template size
TEMPLATE_SIZE=$(du -h "${FINAL_TEMPLATE}" | cut -f1)

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  ✅ Pop!_OS + COSMIC + RustDesk Template Complete                   ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 Template Location:"
echo "   agentReagents: ${FINAL_TEMPLATE}"
echo "   libvirt:       /var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2"
echo "   Size:          ${TEMPLATE_SIZE}"
echo ""
echo "📝 Template Contents:"
echo "   • Pop!_OS 22.04 base"
echo "   • COSMIC desktop (Wayland compositor)"
echo "   • RustDesk 1.2.3 pre-installed"
echo "   • GDM display manager"
echo "   • Default user: iontest / password: iontest123"
echo ""
echo "Ready to use with:"
echo "   cd ../benchScale"
echo "   ./scripts/create-lab.sh --topology ecoprimals-tower-2node --name test"
echo ""

