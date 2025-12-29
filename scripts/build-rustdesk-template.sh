#!/bin/bash
# build-rustdesk-template.sh - Create a RustDesk template image for agentReagents
#
# This script creates a ready-to-use VM template with RustDesk pre-installed
# Following primal philosophy: build once, use many times

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_DIR="$(dirname "$SCRIPT_DIR")"
IMAGE_DIR="$REAGENTS_DIR/images"
TEMPLATE_DIR="$IMAGE_DIR/templates"
CLOUD_DIR="$IMAGE_DIR/cloud"
DEB_DIR="$REAGENTS_DIR/debs/remote-desktop"

# Configuration
TEMPLATE_NAME="rustdesk-ubuntu-22.04-template"
BASE_IMAGE="$CLOUD_DIR/ubuntu-22.04-server-cloudimg-amd64.img"
RUSTDESK_DEB="$DEB_DIR/rustdesk-1.2.3-x86_64.deb"
VM_NAME="template-builder-$$"
MEMORY="2048"
VCPUS="2"
DISK_SIZE="15G"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                                                                        ║${NC}"
echo -e "${BLUE}║          🏗️  RustDesk Template Image Builder                          ║${NC}"
echo -e "${BLUE}║          agentReagents Template Creation                              ║${NC}"
echo -e "${BLUE}║                                                                        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Create directories
mkdir -p "$TEMPLATE_DIR"
mkdir -p "$IMAGE_DIR/intermediates"

# Verify prerequisites
echo -e "${GREEN}✓${NC} Checking prerequisites..."
if [ ! -f "$BASE_IMAGE" ]; then
    echo "❌ Base image not found: $BASE_IMAGE"
    exit 1
fi

if [ ! -f "$RUSTDESK_DEB" ]; then
    echo "❌ RustDesk .deb not found: $RUSTDESK_DEB"
    exit 1
fi

echo -e "${GREEN}✓${NC} Base image: $(basename $BASE_IMAGE)"
echo -e "${GREEN}✓${NC} RustDesk .deb: $(basename $RUSTDESK_DEB)"
echo ""

# Create working directory
WORK_DIR="/tmp/template-builder-$$"
mkdir -p "$WORK_DIR"
cd "$WORK_DIR"

echo -e "${BLUE}📊 Phase 1/6: Preparing disk image${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Copy base image to libvirt location first
BASE_COPY="/var/lib/libvirt/images/$(basename $BASE_IMAGE)"
if [ ! -f "$BASE_COPY" ]; then
    echo "Copying base image to libvirt directory..."
    sudo cp "$BASE_IMAGE" "$BASE_COPY"
    echo -e "${GREEN}✓${NC} Base image copied"
fi

# Create disk from base image
DISK_PATH="/var/lib/libvirt/images/${VM_NAME}.qcow2"
echo "Creating disk from base image..."
sudo qemu-img create -f qcow2 -F qcow2 -b "$BASE_COPY" "$DISK_PATH" "$DISK_SIZE"
sudo qemu-img resize "$DISK_PATH" "$DISK_SIZE"
echo -e "${GREEN}✓${NC} Disk created: $DISK_PATH"
echo ""

# Generate SSH key for provisioning
echo -e "${BLUE}📊 Phase 2/6: Generating SSH credentials${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

SSH_KEY="$WORK_DIR/template_key"
ssh-keygen -t rsa -b 2048 -f "$SSH_KEY" -N "" -C "template-builder"
SSH_PUB=$(cat "${SSH_KEY}.pub")
echo -e "${GREEN}✓${NC} SSH key generated"
echo ""

# Create cloud-init with password fallback for reliability
echo -e "${BLUE}📊 Phase 3/6: Creating cloud-init configuration${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Generate password hash
PASSWORD="template123"
PASSWORD_HASH=$(openssl passwd -6 "$PASSWORD")

cat > "$WORK_DIR/user-data" <<EOF
#cloud-config
users:
  - name: ubuntu
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    lock_passwd: false
    passwd: $PASSWORD_HASH
    ssh_authorized_keys:
      - $SSH_PUB

ssh_pwauth: true
disable_root: false

packages:
  - ubuntu-desktop-minimal
  - xrdp
  - wget
  - curl
  - net-tools

package_update: true
package_upgrade: true

runcmd:
  - systemctl enable ssh
  - systemctl start ssh
  - systemctl enable xrdp
  - echo "Template provisioning complete" > /tmp/cloud-init-done

power_state:
  mode: reboot
  timeout: 300
  condition: true
EOF

cat > "$WORK_DIR/meta-data" <<EOF
instance-id: template-builder-$$
local-hostname: template-builder
EOF

echo -e "${GREEN}✓${NC} Cloud-init configured with both SSH key and password auth"
echo ""

# Create cloud-init ISO
echo "Creating cloud-init ISO..."
sudo genisoimage -output "$WORK_DIR/cidata.iso" -volid cidata -joliet -rock "$WORK_DIR/user-data" "$WORK_DIR/meta-data"
ISO_PATH="/var/lib/libvirt/images/${VM_NAME}-cidata.iso"
sudo cp "$WORK_DIR/cidata.iso" "$ISO_PATH"
echo -e "${GREEN}✓${NC} Cloud-init ISO created"
echo ""

# Create and start VM
echo -e "${BLUE}📊 Phase 4/6: Provisioning VM (5-10 minutes)${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

sudo virt-install \
    --name "$VM_NAME" \
    --memory "$MEMORY" \
    --vcpus "$VCPUS" \
    --disk "path=$DISK_PATH,format=qcow2" \
    --disk "path=$ISO_PATH,device=cdrom" \
    --os-variant ubuntu22.04 \
    --network network=default \
    --graphics vnc,listen=0.0.0.0 \
    --noautoconsole \
    --import

echo -e "${GREEN}✓${NC} VM started: $VM_NAME"
echo ""

# Wait for IP
echo "Waiting for VM to get IP address..."
sleep 30
VM_IP=""
for i in {1..12}; do
    VM_IP=$(virsh domifaddr "$VM_NAME" 2>/dev/null | grep -oE "192\.168\.[0-9]+\.[0-9]+" | head -1 || echo "")
    if [ -n "$VM_IP" ]; then
        break
    fi
    echo "  Attempt $i/12..."
    sleep 10
done

if [ -z "$VM_IP" ]; then
    echo "❌ Failed to get VM IP"
    virsh destroy "$VM_NAME" 2>/dev/null || true
    virsh undefine "$VM_NAME" 2>/dev/null || true
    exit 1
fi

echo -e "${GREEN}✓${NC} VM IP: $VM_IP"
echo ""

# Wait for SSH (cloud-init takes time for desktop install)
echo "Waiting for SSH and cloud-init to complete (this may take 5-10 minutes for desktop)..."
SSH_OPTS="-i $SSH_KEY -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=10"

for i in {1..60}; do
    if ssh $SSH_OPTS ubuntu@$VM_IP 'test -f /tmp/cloud-init-done' 2>/dev/null; then
        echo -e "${GREEN}✓${NC} Cloud-init complete!"
        break
    fi
    echo -n "."
    sleep 10
done
echo ""

# Install RustDesk
echo -e "${BLUE}📊 Phase 5/6: Installing RustDesk${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo "Copying RustDesk .deb to VM..."
scp $SSH_OPTS "$RUSTDESK_DEB" ubuntu@$VM_IP:/tmp/rustdesk.deb

echo "Installing RustDesk..."
ssh $SSH_OPTS ubuntu@$VM_IP 'sudo DEBIAN_FRONTEND=noninteractive dpkg -i /tmp/rustdesk.deb || true'
ssh $SSH_OPTS ubuntu@$VM_IP 'sudo DEBIAN_FRONTEND=noninteractive apt-get install -f -y'

echo "Configuring RustDesk..."
ssh $SSH_OPTS ubuntu@$VM_IP 'rustdesk --password template123'

echo -e "${GREEN}✓${NC} RustDesk installed and configured"
echo ""

# Clean and prepare for template use
echo "Cleaning VM for template use..."
ssh $SSH_OPTS ubuntu@$VM_IP 'sudo cloud-init clean --logs --seed'
ssh $SSH_OPTS ubuntu@$VM_IP 'sudo rm -f /tmp/*'
ssh $SSH_OPTS ubuntu@$VM_IP 'sudo apt-get clean'
ssh $SSH_OPTS ubuntu@$VM_IP 'sudo history -c'

echo -e "${GREEN}✓${NC} VM cleaned for template use"
echo ""

# Shutdown VM
echo "Shutting down VM..."
virsh shutdown "$VM_NAME"
sleep 20

# Wait for shutdown
for i in {1..30}; do
    if ! virsh list --all | grep -q "$VM_NAME.*running"; then
        break
    fi
    sleep 2
done

# Save template
echo -e "${BLUE}📊 Phase 6/6: Saving template image${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

TEMPLATE_PATH="$TEMPLATE_DIR/${TEMPLATE_NAME}.qcow2"
sudo cp "$DISK_PATH" "$TEMPLATE_PATH"
sudo chown $USER:$USER "$TEMPLATE_PATH"

# Save intermediate (raw disk before cleanup)
INTERMEDIATE_PATH="$IMAGE_DIR/intermediates/${TEMPLATE_NAME}-intermediate-$(date +%Y%m%d-%H%M%S).qcow2"
sudo cp "$DISK_PATH" "$INTERMEDIATE_PATH"
sudo chown $USER:$USER "$INTERMEDIATE_PATH"

echo -e "${GREEN}✓${NC} Template saved: $TEMPLATE_PATH"
echo -e "${GREEN}✓${NC} Intermediate saved: $INTERMEDIATE_PATH"
echo ""

# Cleanup
echo "Cleaning up..."
virsh undefine "$VM_NAME" 2>/dev/null || true
sudo rm -f "$DISK_PATH" "$ISO_PATH"
rm -rf "$WORK_DIR"

echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                                                                        ║${NC}"
echo -e "${BLUE}║          ✅ Template Image Created Successfully                        ║${NC}"
echo -e "${BLUE}║                                                                        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "📁 Template: $TEMPLATE_PATH"
echo "📁 Intermediate: $INTERMEDIATE_PATH"
echo ""
echo "🔐 Default credentials:"
echo "   Username: ubuntu"
echo "   Password: template123"
echo ""
echo "📝 Update MANIFEST.md:"
echo "   $(basename $TEMPLATE_PATH) | RustDesk Template (Ubuntu 22.04 + Desktop) | $(date)"
echo ""
echo "🚀 Ready for use with benchScale!"

