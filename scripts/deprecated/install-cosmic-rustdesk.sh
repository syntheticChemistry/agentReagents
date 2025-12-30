#!/bin/bash
# Install COSMIC Desktop + RustDesk on Ubuntu 24.04
# Run this INSIDE the Ubuntu VM via SSH

set -e

echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║  Installing COSMIC Desktop + RustDesk                                   ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""

# Add System76 COSMIC repository
echo "📦 Adding System76 COSMIC repository..."
curl -fsSL https://apt.system76.com/signing-key.asc | gpg --dearmor | sudo tee /etc/apt/keyrings/system76.gpg > /dev/null
echo "deb [signed-by=/etc/apt/keyrings/system76.gpg] https://apt.system76.com/cosmic noble main" | sudo tee /etc/apt/sources.list.d/system76-cosmic.list

echo ""
echo "🔄 Updating package lists..."
sudo apt update

echo ""
echo "📥 Installing COSMIC Desktop (this takes 10-15 minutes)..."
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
    cosmic-edit

echo ""
echo "📥 Installing RustDesk..."
cd /tmp
wget -q https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb
sudo DEBIAN_FRONTEND=noninteractive apt install -y -f ./rustdesk-1.2.3-x86_64.deb || true
sudo DEBIAN_FRONTEND=noninteractive apt install -y -f
rm -f rustdesk-1.2.3-x86_64.deb

echo ""
echo "⚙️  Configuring COSMIC..."
sudo systemctl enable cosmic-greeter
sudo systemctl set-default graphical.target

# Configure auto-login
sudo mkdir -p /etc/cosmic-greeter
sudo tee /etc/cosmic-greeter/auto-login.conf > /dev/null << 'AUTOCONF'
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=ubuntu
AUTOCONF

# Configure RustDesk to auto-start
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

echo ""
echo "✅ Installation complete!"
echo ""
echo "╔══════════════════════════════════════════════════════════════════════════╗"
echo "║  Reboot required to start COSMIC Desktop                                ║"
echo "╚══════════════════════════════════════════════════════════════════════════╝"
echo ""
echo "Run: sudo reboot"
echo ""
echo "After reboot, access via VNC to see COSMIC desktop!"
echo ""

