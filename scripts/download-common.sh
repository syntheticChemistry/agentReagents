#!/bin/bash
# Quick Download Script for Common Resources

set -e

AGENT_REAGENTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "╔═══════════════════════════════════════════════════════════════════════╗"
echo "║                                                                       ║"
echo "║              agentReagents Quick Download Helper                      ║"
echo "║                                                                       ║"
echo "╚═══════════════════════════════════════════════════════════════════════╝"
echo ""

# Function to download with progress
download_with_progress() {
    local url="$1"
    local dest="$2"
    local desc="$3"
    
    echo "📥 Downloading: $desc"
    echo "   URL: $url"
    echo "   Dest: $dest"
    echo ""
    
    wget -P "$dest" "$url" --progress=bar:force 2>&1 | tail -3
    echo "✅ Downloaded successfully!"
    echo ""
}

# Function to add to manifest
add_to_manifest() {
    local file="$1"
    local category="$2"
    local desc="$3"
    local source="$4"
    
    echo "$file | $category | $desc | $(date +%Y-%m-%d) | $source" >> "$AGENT_REAGENTS_DIR/docs/MANIFEST.md"
}

# Function to add checksum
add_checksum() {
    local filepath="$1"
    
    if [ -f "$filepath" ]; then
        sha256sum "$filepath" >> "$AGENT_REAGENTS_DIR/docs/CHECKSUMS.md"
    fi
}

echo "What would you like to download?"
echo ""
echo "1) Ubuntu 22.04 Cloud Image (cloud-init enabled)"
echo "2) RustDesk 1.2.3 (.deb)"
echo "3) Pop!_OS 22.04 ISO (NVIDIA)"
echo "4) All of the above"
echo "5) Custom URL"
echo ""
read -p "Enter choice [1-5]: " choice

case $choice in
    1)
        mkdir -p "$AGENT_REAGENTS_DIR/images/cloud"
        download_with_progress \
            "https://cloud-images.ubuntu.com/releases/22.04/release/ubuntu-22.04-server-cloudimg-amd64.img" \
            "$AGENT_REAGENTS_DIR/images/cloud" \
            "Ubuntu 22.04 Cloud Image"
        
        add_to_manifest \
            "ubuntu-22.04-server-cloudimg-amd64.img" \
            "images/cloud" \
            "Ubuntu 22.04 cloud-init enabled" \
            "cloud-images.ubuntu.com"
        
        add_checksum "$AGENT_REAGENTS_DIR/images/cloud/ubuntu-22.04-server-cloudimg-amd64.img"
        ;;
        
    2)
        mkdir -p "$AGENT_REAGENTS_DIR/debs/remote-desktop"
        download_with_progress \
            "https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb" \
            "$AGENT_REAGENTS_DIR/debs/remote-desktop" \
            "RustDesk 1.2.3"
        
        add_to_manifest \
            "rustdesk-1.2.3-x86_64.deb" \
            "debs/remote-desktop" \
            "RustDesk v1.2.3 remote desktop" \
            "github.com/rustdesk"
        
        add_checksum "$AGENT_REAGENTS_DIR/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb"
        ;;
        
    3)
        mkdir -p "$AGENT_REAGENTS_DIR/isos"
        download_with_progress \
            "https://iso.pop-os.org/22.04/amd64/nvidia/22/pop-os_22.04_amd64_nvidia_22.iso" \
            "$AGENT_REAGENTS_DIR/isos" \
            "Pop!_OS 22.04 NVIDIA"
        
        add_to_manifest \
            "pop-os_22.04_amd64_nvidia_22.iso" \
            "isos" \
            "Pop!_OS 22.04 with NVIDIA drivers" \
            "pop.system76.com"
        
        add_checksum "$AGENT_REAGENTS_DIR/isos/pop-os_22.04_amd64_nvidia_22.iso"
        ;;
        
    4)
        echo "Downloading all common resources..."
        "$0" <<< "1"
        "$0" <<< "2"
        "$0" <<< "3"
        ;;
        
    5)
        read -p "Enter URL: " custom_url
        read -p "Enter category (bins/tars/debs/isos/images): " category
        read -p "Enter description: " desc
        
        mkdir -p "$AGENT_REAGENTS_DIR/$category"
        download_with_progress "$custom_url" "$AGENT_REAGENTS_DIR/$category" "$desc"
        
        filename=$(basename "$custom_url")
        add_to_manifest "$filename" "$category" "$desc" "$custom_url"
        add_checksum "$AGENT_REAGENTS_DIR/$category/$filename"
        ;;
        
    *)
        echo "Invalid choice"
        exit 1
        ;;
esac

echo ""
echo "═══════════════════════════════════════════════════════════════════════"
echo "✅ Download complete!"
echo ""
echo "View manifest: cat $AGENT_REAGENTS_DIR/docs/MANIFEST.md"
echo "Verify checksums: cd $AGENT_REAGENTS_DIR && sha256sum -c docs/CHECKSUMS.md --ignore-missing"
echo "═══════════════════════════════════════════════════════════════════════"

