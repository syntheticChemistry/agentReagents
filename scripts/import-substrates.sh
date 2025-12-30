#!/bin/bash
# Import ionChannel substrates on a new system
set -e

IMPORT_SOURCE="${1:-$HOME/substrate-imports}"
IMAGES_DIR="/var/lib/libvirt/images"

echo "=== ionChannel Substrate Importer ==="
echo ""

if [ ! -d "$IMPORT_SOURCE" ]; then
    echo "❌ Import directory not found: $IMPORT_SOURCE"
    echo ""
    echo "Usage: $0 <import-directory>"
    echo "Example: $0 ~/substrate-imports"
    exit 1
fi

echo "Import source: $IMPORT_SOURCE"
echo "Destination: $IMAGES_DIR"
echo ""

# Check for checksums
CHECKSUM_FILE=$(ls "$IMPORT_SOURCE"/checksums-*.txt 2>/dev/null | head -1)
if [ -n "$CHECKSUM_FILE" ]; then
    echo "=== Verifying Checksums ==="
    cd "$IMPORT_SOURCE"
    if sha256sum -c "$CHECKSUM_FILE"; then
        echo "  ✅ All checksums verified"
    else
        echo "  ❌ Checksum verification failed!"
        exit 1
    fi
    echo ""
fi

# Function to import image
import_image() {
    local pattern=$1
    local dest_name=$2
    
    local source=$(ls "$IMPORT_SOURCE"/$pattern 2>/dev/null | head -1)
    if [ -z "$source" ]; then
        echo "  ⚠️  Skipping $dest_name (not found: $pattern)"
        return
    fi
    
    echo "Importing $dest_name..."
    echo "  Source: $(basename "$source")"
    
    # Copy to libvirt images directory
    sudo cp "$source" "$IMAGES_DIR/$dest_name"
    
    # Set proper ownership and permissions
    sudo chown libvirt-qemu:kvm "$IMAGES_DIR/$dest_name"
    sudo chmod 644 "$IMAGES_DIR/$dest_name"
    
    local size=$(du -h "$IMAGES_DIR/$dest_name" | cut -f1)
    echo "  ✅ Imported ($size)"
}

echo "=== Importing Substrates ==="
echo ""

# Import Ubuntu baseline
echo "Ubuntu 24.04 Baseline:"
import_image "ubuntu24-baseline-*.qcow2" "ionchannel-ubuntu24-baseline.qcow2"
import_image "ubuntu24-intermediate-*.qcow2" "ubuntu24-baseline-configured.qcow2"
echo ""

# Import COSMIC
echo "Pop!_OS COSMIC:"
import_image "popos-cosmic-[0-9]*.qcow2" "ionchannel-popos-cosmic.qcow2"
import_image "popos-cosmic-presetup-*.qcow2" "popos-cosmic-presetup.qcow2"
import_image "popos-cosmic-configured-*.qcow2" "popos-cosmic-configured.qcow2"
echo ""

echo "=== Import Complete ==="
echo ""
echo "Imported substrates:"
ls -lh "$IMAGES_DIR"/ionchannel-*.qcow2 2>/dev/null | awk '{print "  " $9, "(" $5 ")"}'
ls -lh "$IMAGES_DIR"/*-intermediate*.qcow2 2>/dev/null | awk '{print "  " $9, "(" $5 ")"}'
ls -lh "$IMAGES_DIR"/popos-cosmic-*.qcow2 2>/dev/null | awk '{print "  " $9, "(" $5 ")"}'
echo ""
echo "Next steps:"
echo "  1. Verify: cd ../benchScale && cargo run --example verify_substrate"
echo "  2. Test: cd ../ionChannel && cargo run --bin baseline-validation"
echo ""

