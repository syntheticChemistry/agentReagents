#!/bin/bash
# Download software packages

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEBS_DIR="${REAGENTS_ROOT}/debs/remote-desktop"

mkdir -p "${DEBS_DIR}"

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  Downloading Packages                                                ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# RustDesk
if [ ! -f "${DEBS_DIR}/rustdesk-1.2.3-x86_64.deb" ]; then
    echo "📦 Downloading RustDesk 1.2.3 (~18MB)..."
    cd "${DEBS_DIR}"
    wget -c --progress=bar:force:noscroll \
        -O rustdesk-1.2.3-x86_64.deb \
        https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb
    echo "✅ RustDesk downloaded"
else
    echo "✓ RustDesk already exists"
fi

echo ""
echo "✅ All packages ready!"
ls -lh "${DEBS_DIR}"/*.deb

