#!/bin/bash
# Export ionChannel substrates for transfer/backup
set -e

EXPORT_DIR="${SUBSTRATE_EXPORT_DIR:-$HOME/substrate-exports}"
DATE=$(date +%Y%m%d)
IMAGES_DIR="/var/lib/libvirt/images"

echo "=== ionChannel Substrate Exporter ==="
echo ""
echo "Export directory: $EXPORT_DIR"
echo "Date: $DATE"
echo ""

# Create export directory
mkdir -p "$EXPORT_DIR"

# Function to export image
export_image() {
    local source=$1
    local name=$2
    
    if [ ! -f "$IMAGES_DIR/$source" ]; then
        echo "⚠️  Skipping $name (not found)"
        return
    fi
    
    echo "Exporting $name..."
    local dest="$EXPORT_DIR/${name}-${DATE}.qcow2"
    
    qemu-img convert -c -O qcow2 \
        "$IMAGES_DIR/$source" \
        "$dest"
    
    local size=$(du -h "$dest" | cut -f1)
    echo "  ✅ $name exported ($size)"
}

# Export Ubuntu substrate
echo "=== Ubuntu 24.04 Baseline ==="
export_image "ionchannel-ubuntu24-baseline.qcow2" "ubuntu24-baseline"
export_image "ubuntu24-baseline-configured.qcow2" "ubuntu24-intermediate"
echo ""

# Export COSMIC substrate
echo "=== Pop!_OS COSMIC ==="
export_image "ionchannel-popos-cosmic.qcow2" "popos-cosmic"
export_image "popos-cosmic-presetup.qcow2" "popos-cosmic-presetup"
export_image "popos-cosmic-configured.qcow2" "popos-cosmic-configured"
echo ""

# Generate checksums
echo "=== Generating Checksums ==="
cd "$EXPORT_DIR"
sha256sum *-${DATE}.qcow2 > checksums-${DATE}.txt 2>/dev/null || true
echo "  ✅ Checksums saved to checksums-${DATE}.txt"
echo ""

# Generate manifest
echo "=== Creating Manifest ==="
cat > "substrate-manifest-${DATE}.yaml" <<EOF
# ionChannel Validation Substrates
# Generated: $(date -Iseconds)
# Export Date: $DATE

substrates:
EOF

for img in *-${DATE}.qcow2; do
    [ -f "$img" ] || continue
    local size=$(du -h "$img" | cut -f1)
    local checksum=$(sha256sum "$img" | cut -d' ' -f1)
    local name=$(basename "$img" "-${DATE}.qcow2")
    
    cat >> "substrate-manifest-${DATE}.yaml" <<EOF
  - name: $name
    file: $img
    size: $size
    checksum: $checksum
    export_date: $DATE
EOF
done

echo "  ✅ Manifest saved to substrate-manifest-${DATE}.yaml"
echo ""

# Summary
echo "=== Export Summary ==="
echo ""
echo "Location: $EXPORT_DIR"
echo "Files exported:"
ls -lh "$EXPORT_DIR"/*-${DATE}* 2>/dev/null | awk '{print "  " $9, "(" $5 ")"}'
echo ""
echo "Total size:"
du -sh "$EXPORT_DIR"/*-${DATE}.qcow2 2>/dev/null | awk '{sum+=$1} END {print "  " sum}'
echo ""
echo "Next steps:"
echo "  1. Verify: sha256sum -c $EXPORT_DIR/checksums-${DATE}.txt"
echo "  2. Transfer to another system"
echo "  3. Import: ./scripts/import-substrates.sh /path/to/exports"
echo ""

