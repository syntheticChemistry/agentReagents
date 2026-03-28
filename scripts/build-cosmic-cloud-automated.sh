#!/bin/bash
# Build Pop!_OS/COSMIC Template using Cloud-Init (Automated)
# Much faster than manual installation!

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
source "$SCRIPT_DIR/../configs/defaults.env" 2>/dev/null || source "${REAGENTS_ROOT:-$(dirname "$SCRIPT_DIR")}/configs/defaults.env" 2>/dev/null || true

LIBVIRT_IMAGES="${LIBVIRT_IMAGES:-/var/lib/libvirt/images}"

# Use Ubuntu 24.04 cloud image as base, then add COSMIC
RUSTDESK_DEB="${REAGENTS_ROOT}/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb"
VM_NAME="popos-cosmic-cloud-builder"
WORK_DISK="${LIBVIRT_IMAGES}/${VM_NAME}.qcow2"
FINAL_TEMPLATE="${LIBVIRT_IMAGES}/popos-cosmic-rustdesk-template.qcow2"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Building Pop!_OS + COSMIC Template (Cloud-Init Automated)          ║"
echo "║  Using Ubuntu base + System76 COSMIC packages                       ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Download Ubuntu 24.04 cloud image if we don't have it
UBUNTU_24_CLOUD="${REAGENTS_ROOT}/images/cloud/${CLOUD_IMAGE_UBUNTU_2404:-ubuntu-24.04-server-cloudimg-amd64.img}"
if [ ! -f "${UBUNTU_24_CLOUD}" ]; then
    echo "📥 Downloading Ubuntu 24.04 cloud image..."
    mkdir -p "${REAGENTS_ROOT}/images/cloud"
    wget -O "${UBUNTU_24_CLOUD}" \
        https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
    echo "✅ Downloaded Ubuntu 24.04 cloud image"
fi

BASE_IMAGE="${UBUNTU_24_CLOUD}"

echo "✅ Using cloud image: $(basename "${BASE_IMAGE}")"
echo "✅ RustDesk: $(basename "${RUSTDESK_DEB}")"
echo ""

# Copy base image to libvirt directory
echo "📋 Preparing base image..."
sudo cp "${BASE_IMAGE}" "${LIBVIRT_IMAGES}/ubuntu-24-base.qcow2"
sudo chown libvirt-qemu:kvm "${LIBVIRT_IMAGES}/ubuntu-24-base.qcow2"

# Create working disk from base
echo "💿 Creating 25GB working disk..."
sudo qemu-img create -f qcow2 -b "${LIBVIRT_IMAGES}/ubuntu-24-base.qcow2" -F qcow2 "${WORK_DISK}" 25G
sudo qemu-img resize "${WORK_DISK}" 25G

# Copy RustDesk to accessible location
echo "📦 Preparing RustDesk package..."
sudo cp "${RUSTDESK_DEB}" "${LIBVIRT_IMAGES}/rustdesk.deb"

# Generate SSH key
TEMP_DIR=$(mktemp -d)
SSH_KEY="${TEMP_DIR}/id_rsa"
echo "🔑 Generating SSH key..."
ssh-keygen -t rsa -b 2048 -f "${SSH_KEY}" -N "" -C "cosmic-builder" >/dev/null 2>&1
SSH_PUB_KEY=$(cat "${SSH_KEY}.pub")

# Create comprehensive cloud-init user-data
USER_DATA="${TEMP_DIR}/user-data"
cat > "${USER_DATA}" << EOF
#cloud-config
users:
  - name: ${VM_USER}
    groups: users, admin, sudo
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    ssh_authorized_keys:
      - ${SSH_PUB_KEY}

chpasswd:
  list: |
    ${VM_USER}:${VM_PASSWORD}
  expire: false

package_update: true
package_upgrade: true

packages:
  - openssh-server
  - wget
  - curl
  - software-properties-common
  - ubuntu-desktop-minimal

runcmd:
  # Install COSMIC desktop from System76
  - echo "Installing COSMIC desktop..."
  - add-apt-repository -y ppa:system76/cosmic
  - apt-get update
  - DEBIAN_FRONTEND=noninteractive apt-get install -y cosmic-session cosmic-comp cosmic-greeter
  
  # Set COSMIC as default session
  - echo "cosmic" > /etc/X11/default-display-manager
  
  # Install RustDesk
  - echo "Installing RustDesk..."
  - cp "${LIBVIRT_IMAGES}/rustdesk.deb" /tmp/rustdesk.deb
  - DEBIAN_FRONTEND=noninteractive apt-get install -y --fix-broken /tmp/rustdesk.deb || true
  - apt-get install -f -y
  
  # Configure RustDesk auto-start
  - mkdir -p /home/${VM_USER}/.config/autostart
  - |
    cat > /home/${VM_USER}/.config/autostart/rustdesk.desktop << 'RUSTDESK_EOF'
    [Desktop Entry]
    Type=Application
    Name=RustDesk
    Exec=rustdesk
    Hidden=false
    NoDisplay=false
    X-GNOME-Autostart-enabled=true
    RUSTDESK_EOF
  - chown -R ${VM_USER}:${VM_USER} /home/${VM_USER}/.config
  
  # Clean up
  - rm -f /tmp/rustdesk.deb
  - apt-get clean
  - cloud-init clean --logs

power_state:
  mode: poweroff
  timeout: 600
  message: "Template build complete"
EOF

# Create meta-data
META_DATA="${TEMP_DIR}/meta-data"
cat > "${META_DATA}" << EOF
instance-id: ${VM_NAME}
local-hostname: cosmic-template
EOF

echo "🚀 Creating VM with cloud-init (automated)..."
echo "   This will take 20-30 minutes for package installation..."
echo "   VM will automatically shutdown when complete."
echo ""

# Create VM
sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk path="${WORK_DISK}",format=qcow2 \
    --os-variant ubuntu24.04 \
    --network network=default \
    --graphics vnc,listen=0.0.0.0 \
    --noautoconsole \
    --import \
    --cloud-init user-data="${USER_DATA}",meta-data="${META_DATA}"

echo "⏳ Waiting for VM to complete setup and shutdown..."
echo "   You can monitor with: sudo virsh console ${VM_NAME}"
echo ""

# Wait for VM to shutdown
TIMEOUT=2400  # 40 minutes
ELAPSED=0
while sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; do
    sleep 10
    ELAPSED=$((ELAPSED + 10))
    if [ $ELAPSED -ge $TIMEOUT ]; then
        echo "❌ Timeout. Check with: sudo virsh console ${VM_NAME}"
        exit 1
    fi
    if [ $((ELAPSED % 60)) -eq 0 ]; then
        echo "   ... $((ELAPSED / 60)) minutes elapsed (COSMIC + RustDesk installing)"
    fi
done

echo "✅ VM setup completed and shut down"
echo ""

# Finalize
echo "🗜️  Compressing template..."
sudo virsh undefine "${VM_NAME}"
sudo qemu-img convert -O qcow2 -c "${WORK_DISK}" "${FINAL_TEMPLATE}"
sudo chown libvirt-qemu:kvm "${FINAL_TEMPLATE}"
sudo chmod 644 "${FINAL_TEMPLATE}"

# Copy to agentReagents
echo "📦 Saving to agentReagents..."
REAGENTS_TEMPLATE="${REAGENTS_ROOT}/images/templates/popos-cosmic-rustdesk-template.qcow2"
mkdir -p "$(dirname "${REAGENTS_TEMPLATE}")"
sudo cp "${FINAL_TEMPLATE}" "${REAGENTS_TEMPLATE}"
sudo chown $(whoami):$(whoami) "${REAGENTS_TEMPLATE}"

# Cleanup
rm -rf "${TEMP_DIR}"
sudo rm -f "${LIBVIRT_IMAGES}/rustdesk.deb"

TEMPLATE_SIZE=$(du -h "${FINAL_TEMPLATE}" | cut -f1)

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  ✅ COSMIC + RustDesk Template Complete (Automated!)                ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📍 Template: ${FINAL_TEMPLATE}"
echo "   Size: ${TEMPLATE_SIZE}"
echo ""
echo "📦 Contents:"
echo "   • Ubuntu 24.04 base"
echo "   • COSMIC desktop (Wayland)"
echo "   • RustDesk 1.2.3"
echo "   • User: ${VM_USER} / ${VM_PASSWORD}"
echo ""
echo "Ready to use:"
echo "   cd ../benchScale"
echo "   ./scripts/create-lab.sh --topology ecoprimals-tower-2node --name test"
echo ""

