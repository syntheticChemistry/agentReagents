#!/bin/bash
# Configure Pop!_OS 24.04 + COSMIC Template

set -e

VM_NAME="popos24-cosmic-template-builder"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Configuring Pop!_OS 24.04 + COSMIC Template                        ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Get VM IP
echo "🔍 Finding VM IP address..."
sleep 5
VM_IP=$(sudo virsh domifaddr "${VM_NAME}" | grep -oP '\d+\.\d+\.\d+\.\d+' | head -1)

if [ -z "${VM_IP}" ]; then
    echo "❌ Could not get VM IP. Is the VM running and connected to network?"
    echo ""
    echo "Try:"
    echo "   sudo virsh domifaddr ${VM_NAME}"
    exit 1
fi

echo "✅ Found VM at: ${VM_IP}"
echo ""

# Get paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTDESK_DEB="${SCRIPT_DIR}/../debs/remote-desktop/rustdesk-1.2.3-x86_64.deb"

echo "📦 Copying RustDesk to VM..."
scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    "${RUSTDESK_DEB}" iontest@${VM_IP}:/tmp/rustdesk.deb

echo "✅ Copied"
echo ""
echo "🔧 Installing and configuring (this may take 2-3 minutes)..."
echo ""

# SSH and configure
ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null iontest@${VM_IP} << 'EOSSH'
    echo "📦 Installing RustDesk..."
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --fix-broken /tmp/rustdesk.deb 2>&1 | grep -v "^Get:" || true
    sudo apt-get install -f -y -qq
    
    echo "⚙️  Configuring RustDesk auto-start..."
    mkdir -p ~/.config/autostart
    cat > ~/.config/autostart/rustdesk.desktop << EOF
[Desktop Entry]
Type=Application
Name=RustDesk
Exec=rustdesk
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
EOF
    
    echo "🔐 Verifying SSH..."
    sudo systemctl enable ssh >/dev/null 2>&1 || true
    sudo systemctl start ssh >/dev/null 2>&1 || true
    
    echo "🧹 Cleaning up..."
    rm -f /tmp/rustdesk.deb
    sudo apt-get clean
    
    echo ""
    echo "✅ Configuration complete!"
EOSSH

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Configuration Complete!                                             ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📝 Next steps:"
echo "   1. Test RustDesk in the VM (should be running)"
echo "   2. Verify COSMIC desktop is working"
echo "   3. Shutdown the VM:"
echo "      • From inside VM: sudo shutdown -h now"
echo "      • Or: sudo virsh shutdown ${VM_NAME}"
echo ""
echo "   4. Finalize template:"
echo "      ./scripts/finalize-popos-24-template.sh"
echo ""

