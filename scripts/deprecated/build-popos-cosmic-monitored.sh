#!/bin/bash
# Build Pop!_OS 24 COSMIC + RustDesk Template Using benchScale
# Properly monitored with no auto-poweroff

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║                                                                          ║"
echo "║  Building Pop!_OS 24 COSMIC + RustDesk Template (Monitored)             ║"
echo "║                                                                          ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""

# Configuration
BASE_IMAGE="${REAGENTS_ROOT}/images/cloud/ubuntu-24.04-server-cloudimg-amd64.img"
VM_NAME="popos-cosmic-builder-$(date +%Y%m%d-%H%M%S)"
FINAL_TEMPLATE="/var/lib/libvirt/images/popos-24-cosmic-rustdesk-template.qcow2"

# Verify base image exists
if [ ! -f "${BASE_IMAGE}" ]; then
    echo "❌ Base image not found: ${BASE_IMAGE}"
    echo "Run: ./scripts/download-cloud-images.sh"
    exit 1
fi

echo "✅ Using Ubuntu 24.04 cloud image as base"
echo ""

# Generate SSH key for access
TEMP_DIR=$(mktemp -d)
SSH_KEY="${TEMP_DIR}/id_rsa"
echo "🔑 Generating SSH key..."
ssh-keygen -t rsa -b 2048 -f "${SSH_KEY}" -N "" -C "cosmic-builder" >/dev/null 2>&1
SSH_PUB_KEY=$(cat "${SSH_KEY}.pub")

# Create cloud-init user-data (NO AUTO-POWEROFF!)
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
  - curl
  - wget
  - gnupg2
  - ca-certificates
  - software-properties-common
  - openssh-server
  - genisoimage

runcmd:
  - systemctl enable ssh
  - systemctl start ssh

# NO power_state - VM stays running!
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

# Copy to libvirt
sudo cp "${CLOUD_INIT_ISO}" /var/lib/libvirt/images/${VM_NAME}-cloud-init.iso
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/${VM_NAME}-cloud-init.iso

# Create disk
WORK_DISK="/var/lib/libvirt/images/${VM_NAME}.qcow2"
echo "💿 Creating working disk (35GB)..."
sudo qemu-img create -f qcow2 "${WORK_DISK}" 35G
sudo qemu-img resize "${WORK_DISK}" 35G

# Format and copy base image to disk
echo "📋 Copying base image..."
sudo qemu-img convert -O qcow2 "${BASE_IMAGE}" "${WORK_DISK}"
sudo qemu-img resize "${WORK_DISK}" 35G

# Create VM with VNC
echo ""
echo "🚀 Creating VM (will stay running for installation)..."
sudo virt-install \
    --name "${VM_NAME}" \
    --memory 4096 \
    --vcpus 2 \
    --disk "${WORK_DISK}",device=disk,bus=virtio \
    --disk /var/lib/libvirt/images/${VM_NAME}-cloud-init.iso,device=cdrom \
    --os-variant ubuntu24.04 \
    --virt-type kvm \
    --graphics vnc,listen=0.0.0.0 \
    --network network=default,model=virtio \
    --import \
    --noautoconsole

echo ""
echo "⏳ Waiting for VM to boot and get IP..."
sleep 10

# Get VM IP
for i in {1..30}; do
    IP=$(sudo virsh domifaddr "${VM_NAME}" | grep ipv4 | awk '{print $4}' | cut -d/ -f1)
    if [ ! -z "$IP" ]; then
        echo "✅ VM IP: $IP"
        break
    fi
    sleep 2
done

if [ -z "$IP" ]; then
    echo "❌ Failed to get VM IP"
    exit 1
fi

# Wait for SSH
echo "⏳ Waiting for SSH..."
for i in {1..30}; do
    if ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -i "${SSH_KEY}" cosmic@${IP} "echo 'SSH ready'" 2>/dev/null; then
        echo "✅ SSH is ready"
        break
    fi
    sleep 2
done

# Get VNC display
VNC_DISPLAY=$(sudo virsh vncdisplay "${VM_NAME}")
VNC_PORT=$((5900 + ${VNC_DISPLAY#:}))

echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║  VM Created and Ready for COSMIC Installation                           ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "VM Name:  ${VM_NAME}"
echo "IP:       ${IP}"
echo "VNC:      localhost:${VNC_PORT}"
echo "SSH:      ssh -i ${SSH_KEY} cosmic@${IP}"
echo ""
echo "Now installing COSMIC Desktop (this takes 10-15 minutes)..."
echo "You can monitor via:"
echo "  • VNC: vncviewer localhost:${VNC_PORT}"
echo "  • SSH: ssh -i ${SSH_KEY} cosmic@${IP}"
echo ""

# Install COSMIC via SSH (monitored!)
echo "📦 Adding COSMIC repository..."
ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -i "${SSH_KEY}" cosmic@${IP} << 'INSTALL_COSMIC'
set -e

echo "Adding System76 COSMIC repository..."
curl -fsSL https://apt.system76.com/signing-key.asc | gpg --dearmor | sudo tee /etc/apt/keyrings/system76.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/system76.gpg] https://apt.system76.com/cosmic noble main" | sudo tee /etc/apt/sources.list.d/system76-cosmic.list

echo "Updating package lists..."
sudo apt update

echo "Installing COSMIC Desktop (this takes time)..."
sudo DEBIAN_FRONTEND=noninteractive apt install -y \
    cosmic-session \
    cosmic-greeter \
    cosmic-comp \
    cosmic-panel \
    cosmic-launcher \
    cosmic-applets \
    cosmic-settings \
    cosmic-files \
    cosmic-term \
    cosmic-edit \
    pipewire \
    wireplumber

echo "Installing RustDesk..."
cd /tmp
wget -q https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb
sudo DEBIAN_FRONTEND=noninteractive apt install -y -f ./rustdesk-1.2.3-x86_64.deb || true
sudo DEBIAN_FRONTEND=noninteractive apt install -y -f
rm -f rustdesk-1.2.3-x86_64.deb

echo "Configuring COSMIC..."
sudo systemctl enable cosmic-greeter
sudo systemctl set-default graphical.target

sudo mkdir -p /etc/cosmic-greeter
sudo tee /etc/cosmic-greeter/auto-login.conf > /dev/null << 'AUTOCONF'
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=cosmic
AUTOCONF

mkdir -p ~/.config/autostart
cat > ~/.config/autostart/rustdesk.desktop << 'RUSTDESK'
[Desktop Entry]
Type=Application
Name=RustDesk
Exec=/usr/bin/rustdesk
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
Comment=RustDesk Remote Desktop
RUSTDESK

echo "Cleaning up..."
sudo apt autoremove -y
sudo apt clean
sudo sync

echo "Installation complete!"
INSTALL_COSMIC

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ COSMIC installation completed successfully!"
    echo ""
    echo "🔄 Rebooting VM to start COSMIC..."
    ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -i "${SSH_KEY}" cosmic@${IP} "sudo reboot" || true
    
    echo "⏳ Waiting 60 seconds for reboot..."
    sleep 60
    
    echo "🎨 VM should now be running COSMIC Desktop!"
    echo "   Check VNC: vncviewer localhost:${VNC_PORT}"
    echo ""
    echo "Press ENTER when you've verified COSMIC is working..."
    read
    
    echo ""
    echo "💾 Shutting down VM to save as template..."
    sudo virsh shutdown "${VM_NAME}"
    
    echo "⏳ Waiting for VM to shut down..."
    for i in {1..30}; do
        if ! sudo virsh domstate "${VM_NAME}" 2>/dev/null | grep -q "running"; then
            echo "✅ VM has shut down"
            break
        fi
        sleep 2
    done
    
    echo "⚡ Optimizing template..."
    sudo virt-sparsify --in-place "${WORK_DISK}"
    
    echo "📦 Finalizing template..."
    sudo cp "${WORK_DISK}" "${FINAL_TEMPLATE}"
    sudo chown libvirt-qemu:kvm "${FINAL_TEMPLATE}"
    sudo chmod 644 "${FINAL_TEMPLATE}"
    
    echo "🧹 Cleaning up builder VM..."
    sudo virsh undefine "${VM_NAME}"
    sudo rm -f "${WORK_DISK}"
    sudo rm -f /var/lib/libvirt/images/${VM_NAME}-cloud-init.iso
    
    rm -rf "${TEMP_DIR}"
    
    echo ""
    echo "╔══════════════════════════════════════════════════════════════════════════╗"
    echo "║                                                                          ║"
    echo "║  ✅ Pop!_OS 24 COSMIC + RustDesk Template Created!                      ║"
    echo "║                                                                          ║"
    echo "╚══════════════════════════════════════════════════════════════════════════╝"
    echo ""
    echo "📍 Template: ${FINAL_TEMPLATE}"
    echo "📊 Size: $(du -h ${FINAL_TEMPLATE} | cut -f1)"
    echo ""
else
    echo ""
    echo "❌ Installation failed!"
    echo "VM is still running for debugging."
    echo "SSH: ssh -i ${SSH_KEY} cosmic@${IP}"
    echo "VNC: vncviewer localhost:${VNC_PORT}"
    echo ""
    echo "After fixing issues, you can manually:"
    echo "  1. sudo virsh shutdown ${VM_NAME}"
    echo "  2. sudo virt-sparsify --in-place ${WORK_DISK}"
    echo "  3. sudo cp ${WORK_DISK} ${FINAL_TEMPLATE}"
    exit 1
fi

