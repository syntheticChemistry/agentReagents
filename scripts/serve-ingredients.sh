#!/bin/bash
# Serve gathered ingredients via local HTTP server
# Makes packages available to VMs at http://192.168.122.1:8080

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
PACKAGES_DIR="${REAGENTS_ROOT}/packages"
PORT=8080

# Check if ingredients have been gathered
if [ ! -d "${PACKAGES_DIR}/apt-cache/archives" ] || [ -z "$(ls -A ${PACKAGES_DIR}/apt-cache/archives/*.deb 2>/dev/null)" ]; then
    echo "❌ No ingredients found!"
    echo ""
    echo "Please gather ingredients first:"
    echo "  ./scripts/gather-ingredients.sh"
    echo ""
    exit 1
fi

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  🍽️  Serving Ingredients (Mise en Place)                             ║"
echo "║  Local package server for airgap-ready VM synthesis                  ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Count available packages
DEB_COUNT=$(find "${PACKAGES_DIR}" -name "*.deb" | wc -l)
TOTAL_SIZE=$(du -sh "${PACKAGES_DIR}" | cut -f1)

echo "📊 Inventory:"
echo "   ${DEB_COUNT} packages available"
echo "   ${TOTAL_SIZE} total size"
echo ""

echo "🌐 Starting local package server..."
echo "   URL: http://192.168.122.1:${PORT}"
echo "   Directory: ${PACKAGES_DIR}"
echo ""
echo "📡 VMs can now install packages from local cache (10-50x faster!)"
echo ""
echo "Press Ctrl+C to stop serving"
echo ""
echo "═══════════════════════════════════════════════════════════════════════"

# Change to packages directory and start server
cd "${PACKAGES_DIR}"
python3 -m http.server "${PORT}"

