#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright © 2024-2025 DataScienceBioLab

#===============================================================================
# STEP 2: Desktop Environment Installation
# 
# Laboratory-grade stepwise synthesis for Ubuntu 24.04 desktop
# This script runs AFTER Step 1 (minimal server) completes successfully
#
# Usage:
#   ./install-desktop-step2.sh [--vnc-only] [--auto-login USERNAME]
#===============================================================================

set -euo pipefail

# Configuration
LOGFILE="/var/log/desktop-install-step2.log"
MARKER_FILE="/root/STEP_2_COMPLETE"
AUTO_LOGIN_USER="${AUTO_LOGIN_USER:-ubuntutest}"
VNC_ONLY="${VNC_ONLY:-false}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')]${NC} $*" | tee -a "$LOGFILE"
}

error() {
    echo -e "${RED}[ERROR]${NC} $*" | tee -a "$LOGFILE" >&2
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $*" | tee -a "$LOGFILE"
}

check_step1() {
    if [[ ! -f "/root/STEP_1_COMPLETE" ]]; then
        error "Step 1 not complete! Run minimal server build first."
        exit 1
    fi
    log "✅ Step 1 verified complete"
}

install_gnome_desktop() {
    log "📦 Installing GNOME desktop (Step 2a)..."
    
    # Update package cache
    log "Updating package cache..."
    apt-get update | tee -a "$LOGFILE"
    
    # Install desktop packages ONE AT A TIME with progress tracking
    local packages=(
        "ubuntu-desktop-minimal"
        "gdm3"
        "gnome-terminal"
        "firefox"
        "gnome-shell-extensions"
    )
    
    for pkg in "${packages[@]}"; do
        log "Installing: $pkg"
        if DEBIAN_FRONTEND=noninteractive apt-get install -y "$pkg" >> "$LOGFILE" 2>&1; then
            log "✅ Installed: $pkg"
        else
            error "Failed to install: $pkg"
            return 1
        fi
    done
    
    log "✅ GNOME desktop installed"
}

configure_gdm() {
    log "⚙️  Configuring GDM3 (Step 2b)..."
    
    # Configure auto-login
    cat > /etc/gdm3/custom.conf <<EOF
[daemon]
AutomaticLoginEnable=true
AutomaticLogin=$AUTO_LOGIN_USER
WaylandEnable=true

[security]

[xdmcp]

[chooser]

[debug]
EOF
    
    log "✅ GDM3 configured for auto-login: $AUTO_LOGIN_USER"
}

enable_desktop_services() {
    log "🔧 Enabling desktop services (Step 2c)..."
    
    systemctl enable gdm3 | tee -a "$LOGFILE"
    
    log "✅ Desktop services enabled"
}

create_step2_marker() {
    log "📝 Creating Step 2 completion marker..."
    
    cat > "$MARKER_FILE" <<EOF
Ubuntu 24.04 Desktop Environment - Step 2 Complete
Date: $(date)
Desktop: GNOME
Display Manager: GDM3
Auto-login: $AUTO_LOGIN_USER
Next: Install applications (RustDesk)
EOF
    
    log "✅ Step 2 marker created"
}

reboot_to_desktop() {
    log "🔄 Rebooting to start desktop environment..."
    log "System will be available at graphical target after reboot"
    
    # Schedule reboot in 10 seconds
    shutdown -r +1 "Rebooting to desktop environment in 1 minute..." | tee -a "$LOGFILE"
}

main() {
    log "═══════════════════════════════════════════════════"
    log "    STEP 2: Desktop Environment Installation"
    log "═══════════════════════════════════════════════════"
    
    # Check prerequisites
    check_step1
    
    # Install desktop (Step 2a)
    install_gnome_desktop
    
    # Configure display manager (Step 2b)
    configure_gdm
    
    # Enable services (Step 2c)
    enable_desktop_services
    
    # Mark completion
    create_step2_marker
    
    log "═══════════════════════════════════════════════════"
    log "✅ STEP 2 COMPLETE!"
    log "═══════════════════════════════════════════════════"
    log ""
    log "Next steps:"
    log "  1. Reboot the system: sudo reboot"
    log "  2. Verify desktop loads"
    log "  3. Run Step 3: ./install-rustdesk-step3.sh"
    log ""
    
    # Offer to reboot
    read -p "Reboot now? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        reboot_to_desktop
    else
        log "Skipping reboot. Run 'sudo reboot' manually when ready."
    fi
}

# Run main function
main "$@"

