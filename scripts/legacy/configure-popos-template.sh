#!/bin/bash
# Configure Pop!_OS Template - Run after OS installation
# This script should be run INSIDE the Pop!_OS VM after installation

set -e

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  Configuring Pop!_OS Template                                       ║"
echo "║  RustDesk + COSMIC + SSH Setup                                      ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# This should be run from the host, but will SSH into the VM
VM_NAME="popos-cosmic-template-builder"
VM_IP=$(virsh domifaddr "${VM_NAME}" | grep -oP '\d+\.\d+\.\d+\.\d+' | head -1)

if [ -z "${VM_IP}" ]; then
    echo "❌ Could not get VM IP. Is it running?"
    exit 1
fi

echo "✅ Found VM at: ${VM_IP}"
echo ""
echo "📦 Copying RustDesk package to VM..."

# Copy RustDesk deb to VM
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTDESK_DEB="${SCRIPT_DIR}/../debs/remote-desktop/rustdesk-1.2.3-x86_64.deb"

scp -o StrictHostKeyChecking=no "${RUSTDESK_DEB}" iontest@${VM_IP}:/tmp/rustdesk.deb

echo "✅ RustDesk copied"
echo ""
echo "🔧 Installing and configuring..."

# SSH into VM and configure
ssh -o StrictHostKeyChecking=no iontest@${VM_IP} << 'EOSSH'
    # Install RustDesk
    echo "📦 Installing RustDesk..."
    sudo apt-get update
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --fix-broken /tmp/rustdesk.deb || true
    sudo apt-get install -f -y
    
    # Configure RustDesk to auto-start
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
    
    # Ensure SSH is enabled
    echo "🔐 Ensuring SSH is enabled..."
    sudo systemctl enable ssh
    sudo systemctl start ssh
    
    # Clean up
    echo "🧹 Cleaning up..."
    rm -f /tmp/rustdesk.deb
    sudo apt-get clean
    
    echo ""
    echo "✅ Configuration complete!"
    echo ""
    echo "🎯 You can now:"
    echo "   1. Test RustDesk (it should be running)"
    echo "   2. Verify COSMIC session is working"
    echo "   3. Shutdown the VM"
    echo "   4. Run: agentReagents/scripts/finalize-popos-template.sh"
EOSSH

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Configuration Complete!                                             ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""
echo "📝 Next steps:"
echo "   1. Verify everything works in the VM"
echo "   2. Shutdown: sudo shutdown -h now"
echo "   3. Run: ./scripts/finalize-popos-template.sh"

