#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright © 2024-2025 DataScienceBioLab

#===============================================================================
# STEP 3: RustDesk Installation
# 
# Final step: Install and configure RustDesk remote desktop
# Requires Step 1 (minimal server) and Step 2 (desktop) to be complete
#
# Usage:
#   ./install-rustdesk-step3.sh [--deb-file PATH] [--version VERSION]
#===============================================================================

set -euo pipefail

# Configuration
LOGFILE="/var/log/rustdesk-install-step3.log"
MARKER_FILE="/root/STEP_3_COMPLETE"
RUSTDESK_DEB="${RUSTDESK_DEB:-/tmp/rustdesk.deb}"
RUSTDESK_VERSION="${RUSTDESK_VERSION:-1.2.3}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $*" | tee -a "$LOGFILE"
}

error() {
    echo -e "${RED}[ERROR]${NC} $*" | tee -a "$LOGFILE" >&2
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $*" | tee -a "$LOGFILE"
}

check_prerequisites() {
    log "🔍 Checking prerequisites..."
    
    if [[ ! -f "/root/STEP_1_COMPLETE" ]]; then
        error "Step 1 not complete!"
        exit 1
    fi
    
    if [[ ! -f "/root/STEP_2_COMPLETE" ]]; then
        error "Step 2 not complete!"
        exit 1
    fi
    
    # Check if desktop is running
    if ! systemctl is-active --quiet gdm3; then
        warn "GDM3 is not running. Desktop may not be started."
    fi
    
    log "✅ Prerequisites verified"
}

install_dependencies() {
    log "📦 Installing RustDesk dependencies..."
    
    apt-get update | tee -a "$LOGFILE"
    
    DEBIAN_FRONTEND=noninteractive apt-get install -y \
        libgtk-3-0 \
        libxcb-shape0 \
        libxcb-xfixes0 \
        libxdo3 \
        libayatana-appindicator3-1 \
        >> "$LOGFILE" 2>&1
    
    log "✅ Dependencies installed"
}

install_rustdesk() {
    log "📥 Installing RustDesk..."
    
    if [[ ! -f "$RUSTDESK_DEB" ]]; then
        error "RustDesk .deb file not found: $RUSTDESK_DEB"
        log "Downloading RustDesk $RUSTDESK_VERSION..."
        
        local url="https://github.com/rustdesk/rustdesk/releases/download/${RUSTDESK_VERSION}/rustdesk-${RUSTDESK_VERSION}-x86_64.deb"
        wget -O "$RUSTDESK_DEB" "$url" | tee -a "$LOGFILE"
    fi
    
    log "Installing RustDesk package..."
    DEBIAN_FRONTEND=noninteractive dpkg -i "$RUSTDESK_DEB" >> "$LOGFILE" 2>&1 || {
        log "Fixing dependencies..."
        apt-get install -f -y >> "$LOGFILE" 2>&1
    }
    
    log "✅ RustDesk installed"
}

configure_rustdesk() {
    log "⚙️  Configuring RustDesk..."
    
    # Create config directory
    mkdir -p /home/ubuntutest/.config/rustdesk
    
    # Basic configuration
    cat > /home/ubuntutest/.config/rustdesk/RustDesk.toml <<EOF
[options]
# Basic RustDesk configuration
direct-server = false
relay-server = ""
api-server = ""
key = ""
EOF
    
    chown -R ubuntutest:ubuntutest /home/ubuntutest/.config/rustdesk
    
    log "✅ RustDesk configured"
}

enable_rustdesk_autostart() {
    log "🚀 Enabling RustDesk autostart..."
    
    mkdir -p /home/ubuntutest/.config/autostart
    
    cat > /home/ubuntutest/.config/autostart/rustdesk.desktop <<EOF
[Desktop Entry]
Type=Application
Name=RustDesk
Exec=/usr/bin/rustdesk
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
Comment=RustDesk Remote Desktop
EOF
    
    chown -R ubuntutest:ubuntutest /home/ubuntutest/.config/autostart
    
    log "✅ RustDesk autostart enabled"
}

create_step3_marker() {
    log "📝 Creating Step 3 completion marker..."
    
    cat > "$MARKER_FILE" <<EOF
Ubuntu 24.04 + RustDesk - Step 3 Complete
Date: $(date)
RustDesk Version: $(rustdesk --version 2>/dev/null || echo "Unknown")
Configuration: Wayland-enabled GNOME + RustDesk

ALL STEPS COMPLETE ✅
This is a control substrate for ionChannel testing.
EOF
    
    log "✅ Step 3 marker created"
}

verify_installation() {
    log "🔍 Verifying installation..."
    
    if command -v rustdesk &> /dev/null; then
        local version=$(rustdesk --version 2>/dev/null || echo "Unknown")
        log "✅ RustDesk installed: $version"
    else
        error "RustDesk binary not found!"
        return 1
    fi
    
    log "✅ Installation verified"
}

main() {
    log "═══════════════════════════════════════════════════"
    log "       STEP 3: RustDesk Installation"
    log "═══════════════════════════════════════════════════"
    
    # Check prerequisites
    check_prerequisites
    
    # Install dependencies
    install_dependencies
    
    # Install RustDesk
    install_rustdesk
    
    # Configure
    configure_rustdesk
    
    # Enable autostart
    enable_rustdesk_autostart
    
    # Verify
    verify_installation
    
    # Mark completion
    create_step3_marker
    
    log "═══════════════════════════════════════════════════"
    log "✅ ALL STEPS COMPLETE!"
    log "═══════════════════════════════════════════════════"
    log ""
    log "Control substrate ready:"
    log "  - Ubuntu 24.04 LTS ✅"
    log "  - GNOME Desktop ✅"
    log "  - Wayland Enabled ✅"
    log "  - RustDesk Installed ✅"
    log ""
    log "Next: Test RustDesk connectivity"
    log ""
}

# Run main function
main "$@"

