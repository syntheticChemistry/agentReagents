#!/bin/bash
# agentReagents Setup Script
# Automated setup for new towers - downloads all required binaries

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REAGENTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
source "$SCRIPT_DIR/../configs/defaults.env" 2>/dev/null || source "${REAGENTS_ROOT:-$(dirname "$SCRIPT_DIR")}/configs/defaults.env" 2>/dev/null || true

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                      ║"
echo "║  agentReagents Setup - Automated Binary Download                    ║"
echo "║  Setting up VM templates and validation resources                   ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Parse arguments
UPDATE_ONLY=false
SKIP_ISOS=false
SKIP_CLOUD=false
SKIP_PACKAGES=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --update)
            UPDATE_ONLY=true
            shift
            ;;
        --skip-isos)
            SKIP_ISOS=true
            shift
            ;;
        --skip-cloud)
            SKIP_CLOUD=true
            shift
            ;;
        --skip-packages)
            SKIP_PACKAGES=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --update          Update scripts only, skip downloads"
            echo "  --skip-isos       Skip ISO downloads (~13GB)"
            echo "  --skip-cloud      Skip cloud image downloads (~2GB)"
            echo "  --skip-packages   Skip package downloads (~18MB)"
            echo "  --help            Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Create directory structure
echo "📁 Creating directory structure..."
mkdir -p "${REAGENTS_ROOT}"/{isos,images/{base,cloud,intermediates,templates},debs/remote-desktop,bins,tars,configs}

# Download ISOs
if [ "$SKIP_ISOS" = false ] && [ "$UPDATE_ONLY" = false ]; then
    echo ""
    echo "📥 Downloading ISOs (~13GB total, ~15-30 minutes)..."
    bash "${SCRIPT_DIR}/download-isos.sh"
else
    echo "⏭️  Skipping ISO downloads"
fi

# Download cloud images
if [ "$SKIP_CLOUD" = false ] && [ "$UPDATE_ONLY" = false ]; then
    echo ""
    echo "📥 Downloading cloud images (~2GB total, ~3-5 minutes)..."
    bash "${SCRIPT_DIR}/download-cloud-images.sh"
else
    echo "⏭️  Skipping cloud image downloads"
fi

# Download packages
if [ "$SKIP_PACKAGES" = false ] && [ "$UPDATE_ONLY" = false ]; then
    echo ""
    echo "📦 Downloading packages (~18MB, ~1 minute)..."
    bash "${SCRIPT_DIR}/download-packages.sh"
else
    echo "⏭️  Skipping package downloads"
fi

# Verify setup
echo ""
echo "🔍 Verifying setup..."
bash "${SCRIPT_DIR}/verify-setup.sh" || true

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  ✅ agentReagents Setup Complete!                                    ║"
echo "║                                                                      ║"
echo "║  Resources Ready:                                                    ║"
echo "║  • ISOs for Pop!_OS 22/24 and Ubuntu 24                             ║"
echo "║  • Cloud images for automated builds                                ║"
echo "║  • RustDesk packages                                                 ║"
echo "║                                                                      ║"
echo "║  Next Steps:                                                         ║"
echo "║  • Build templates: sudo bash scripts/build-cosmic-cloud-automated.sh║"
echo "║  • Run validation: cd ../benchScale && ./scripts/run-tests.sh        ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"

