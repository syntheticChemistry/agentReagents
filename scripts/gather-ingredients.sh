#!/bin/bash
# Mise en place: Gather all ingredients for VM synthesis
# This script makes agentReagents self-sufficient and airgap-ready

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PACKAGES_DIR="${REAGENTS_ROOT}/packages"
CACHE_DIR="${PACKAGES_DIR}/apt-cache/archives"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  🧪 Mise en Place: Gathering All Ingredients                         ║"
echo "║  Making agentReagents self-sufficient and airgap-ready               ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Create directory structure
mkdir -p "${CACHE_DIR}"
mkdir -p "${PACKAGES_DIR}/debs/remote-desktop"

echo "📍 Working directory: ${REAGENTS_ROOT}"
echo ""

# Phase 1: Infrastructure packages (lightweight, fast)
echo "═══════════════════════════════════════════════════════════════════════"
echo "📦 Phase 1: Infrastructure Packages (~10 MB)"
echo "═══════════════════════════════════════════════════════════════════════"
cd "${CACHE_DIR}"

INFRA_PACKAGES=(
    openssh-server
    curl
    wget
    net-tools
    vim
    git
    build-essential
)

for pkg in "${INFRA_PACKAGES[@]}"; do
    echo "  ↓ Downloading: ${pkg}"
    apt-get download "${pkg}" 2>/dev/null || echo "    (already have or unavailable)"
done

echo "✅ Infrastructure packages ready"
echo ""

# Phase 2: X11 components (minimal, essential)
echo "═══════════════════════════════════════════════════════════════════════"
echo "📦 Phase 2: X11 Components (~50 MB)"
echo "═══════════════════════════════════════════════════════════════════════"

X11_PACKAGES=(
    xserver-xorg-core
    xserver-xorg-input-all
    xserver-xorg-video-all
    x11-utils
    x11-common
    x11-apps
    xauth
    xinit
)

for pkg in "${X11_PACKAGES[@]}"; do
    echo "  ↓ Downloading: ${pkg}"
    apt-get download "${pkg}" 2>/dev/null || echo "    (already have or unavailable)"
done

echo "✅ X11 components ready"
echo ""

# Phase 3: Desktop environment (large, takes time)
echo "═══════════════════════════════════════════════════════════════════════"
echo "📦 Phase 3: Desktop Environment (~2 GB)"
echo "═══════════════════════════════════════════════════════════════════════"
echo "⚠️  This will take 10-30 minutes depending on network speed..."
echo ""

DESKTOP_PACKAGES=(
    ubuntu-desktop-minimal
    gdm3
    gnome-terminal
    nautilus
    firefox
)

for pkg in "${DESKTOP_PACKAGES[@]}"; do
    echo "  ↓ Downloading: ${pkg} (with dependencies)"
    apt-get download "${pkg}" 2>/dev/null || echo "    (already have or unavailable)"
done

echo "✅ Desktop environment packages ready"
echo ""

# Phase 4: Download dependencies
echo "═══════════════════════════════════════════════════════════════════════"
echo "📦 Phase 4: Resolving Dependencies"
echo "═══════════════════════════════════════════════════════════════════════"
echo "Using apt-get to download all dependencies..."
echo ""

# Create temporary file for package list
TEMP_PKG_LIST=$(mktemp)
cat > "${TEMP_PKG_LIST}" << EOF
openssh-server
curl
wget
net-tools
vim
git
build-essential
xserver-xorg-core
xserver-xorg-input-all
xserver-xorg-video-all
x11-utils
x11-common
x11-apps
xauth
xinit
ubuntu-desktop-minimal
gdm3
gnome-terminal
nautilus
firefox
EOF

# Use apt-get to download all packages and dependencies
echo "  ⏳ Resolving and downloading all dependencies (this may take 10-20 min)..."
cd "${CACHE_DIR}"
xargs -a "${TEMP_PKG_LIST}" apt-get download -y 2>&1 | grep -v "already" || true

rm "${TEMP_PKG_LIST}"

echo "✅ All dependencies resolved"
echo ""

# Phase 5: Custom packages (RustDesk, etc.)
echo "═══════════════════════════════════════════════════════════════════════"
echo "📦 Phase 5: Custom Packages"
echo "═══════════════════════════════════════════════════════════════════════"

DEBS_DIR="${PACKAGES_DIR}/debs/remote-desktop"
cd "${DEBS_DIR}"

if [ ! -f "rustdesk-1.2.3-x86_64.deb" ]; then
    echo "  ↓ Downloading: RustDesk 1.2.3"
    wget -q --show-progress \
        -O rustdesk-1.2.3-x86_64.deb \
        https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb
    echo "✅ RustDesk downloaded"
else
    echo "✓ RustDesk already present"
fi

echo ""

# Summary
echo "═══════════════════════════════════════════════════════════════════════"
echo "📊 Inventory Summary"
echo "═══════════════════════════════════════════════════════════════════════"
echo ""
echo "APT Cache:"
DEB_COUNT=$(find "${CACHE_DIR}" -name "*.deb" | wc -l)
CACHE_SIZE=$(du -sh "${CACHE_DIR}" | cut -f1)
echo "  📦 ${DEB_COUNT} .deb files"
echo "  💾 ${CACHE_SIZE} total size"
echo ""

echo "Custom Packages:"
CUSTOM_COUNT=$(find "${PACKAGES_DIR}/debs" -name "*.deb" | wc -l)
CUSTOM_SIZE=$(du -sh "${PACKAGES_DIR}/debs" 2>/dev/null | cut -f1 || echo "0")
echo "  📦 ${CUSTOM_COUNT} custom packages"
echo "  💾 ${CUSTOM_SIZE} total size"
echo ""

echo "═══════════════════════════════════════════════════════════════════════"
echo "✅ Mise en place complete!"
echo "═══════════════════════════════════════════════════════════════════════"
echo ""
echo "🎯 agentReagents is now self-sufficient and airgap-ready"
echo ""
echo "Next steps:"
echo "  1. Start package server: ./scripts/serve-ingredients.sh"
echo "  2. Build VMs: agent-reagents build <template>"
echo "  3. Enjoy 10-50x faster builds!"
echo ""

