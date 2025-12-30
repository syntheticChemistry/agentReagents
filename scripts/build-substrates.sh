#!/bin/bash
# Build ionChannel validation substrates
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=== ionChannel Substrate Builder ==="
echo ""

# Check for SSH key
if [ ! -f ~/.ssh/id_rsa.pub ]; then
    echo "❌ No SSH public key found at ~/.ssh/id_rsa.pub"
    echo "   Generate one with: ssh-keygen -t rsa"
    exit 1
fi

SSH_KEY=$(cat ~/.ssh/id_rsa.pub)

# Function to build substrate
build_substrate() {
    local template=$1
    local name=$2
    
    echo "=== Building $name ==="
    echo "Template: $template"
    echo ""
    
    cd "$PROJECT_ROOT"
    cargo run --release -- build \
        "templates/$template" \
        --ssh-key "$SSH_KEY"
    
    echo ""
    echo "✅ $name build complete"
    echo ""
}

# Parse arguments
case "${1:-all}" in
    ubuntu|ubuntu24)
        build_substrate "ionchannel-ubuntu24-baseline.yaml" "Ubuntu 24.04 Baseline"
        ;;
    cosmic|popos)
        echo "⚠️  COSMIC build requires user interaction"
        echo "   You will need to complete GUI setup via VNC"
        echo ""
        read -p "Continue? (y/N): " confirm
        if [[ $confirm =~ ^[Yy]$ ]]; then
            build_substrate "ionchannel-popos-cosmic.yaml" "Pop!_OS COSMIC"
        else
            echo "Cancelled"
            exit 0
        fi
        ;;
    all)
        build_substrate "ionchannel-ubuntu24-baseline.yaml" "Ubuntu 24.04 Baseline"
        echo ""
        echo "════════════════════════════════════════"
        echo ""
        echo "⚠️  Next: COSMIC build (requires user interaction)"
        echo ""
        read -p "Build COSMIC substrate now? (y/N): " confirm
        if [[ $confirm =~ ^[Yy]$ ]]; then
            build_substrate "ionchannel-popos-cosmic.yaml" "Pop!_OS COSMIC"
        else
            echo "Skipping COSMIC. Run './scripts/build-substrates.sh cosmic' to build later"
        fi
        ;;
    *)
        echo "Usage: $0 [ubuntu|cosmic|all]"
        echo ""
        echo "Options:"
        echo "  ubuntu  - Build Ubuntu 24.04 baseline only (fully automated)"
        echo "  cosmic  - Build Pop!_OS COSMIC only (requires GUI setup)"
        echo "  all     - Build both (default)"
        exit 1
        ;;
esac

echo ""
echo "=== Build Complete ==="
echo ""
echo "Substrates created in: /var/lib/libvirt/images/"
echo ""
echo "Next steps:"
echo "  1. Export: ./scripts/export-substrates.sh"
echo "  2. Validate: cd ../ionChannel && cargo run --bin baseline-validation"
echo ""

