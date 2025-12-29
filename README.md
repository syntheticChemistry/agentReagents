# agentReagents - Shared Resources Repository

**Purpose:** Centralized storage for binaries, packages, images, and other artifacts used across syntheticChemistry projects.

**Philosophy:** AI agents can deposit, humans can retrieve, organized LMS-style for easy discovery.

**Git-Friendly:** Scripts and docs are tracked in git. Large binaries (ISOs, images) are downloaded via automated scripts.

---

## 🚀 Quick Start (New Tower Setup)

```bash
# 1. Clone this repository
git clone <repo-url> agentReagents
cd agentReagents

# 2. Run automated setup (downloads all binaries)
bash scripts/setup-reagents.sh

# 3. Verify setup
bash scripts/verify-setup.sh
```

**Done!** All ISOs, images, and packages are downloaded and ready.

See [SETUP.md](SETUP.md) for detailed setup instructions and [ISO_DOWNLOAD_LINKS.md](ISO_DOWNLOAD_LINKS.md) for manual downloads.

---

## 📁 Directory Structure

```
agentReagents/
├── bins/           # Compiled binaries and executables
├── tars/           # Compressed archives (.tar.gz, .tar.xz)
├── debs/           # Debian packages (.deb)
│   └── remote-desktop/
│       └── rustdesk-1.2.3-x86_64.deb
├── isos/           # ISO images for VMs (13GB total)
│   ├── pop-os_22.04_amd64_nvidia_22.iso (3.0GB)
│   ├── pop-os_24.04_amd64_nvidia_22.iso (3.4GB) ⭐
│   └── ubuntu-24.04.3-desktop-amd64.iso (6.0GB)
├── images/         # VM disk images (qcow2, img, raw)
│   ├── base/       # Base OS images
│   ├── templates/  # Pre-built VM templates
│   └── intermediates/  # Snapshot backups
├── configs/        # Configuration files and templates
├── scripts/        # Template builders and utilities
└── docs/           # Documentation and manifests
```

---

## 🎯 Usage Patterns

### For AI Agents

**Depositing artifacts:**
```bash
# Download and store
cd /home/nestgate/Development/syntheticChemistry/agentReagents
wget -P debs/ https://example.com/package.deb

# Organize by category
mv rustdesk-*.deb debs/remote-desktop/
```

**Creating manifests:**
```bash
# Document what was stored
echo "rustdesk-1.2.3-x86_64.deb | RustDesk Remote Desktop | 2024-12-27" >> docs/MANIFEST.md
```

### For Humans

**Finding artifacts:**
```bash
# Browse by type
ls -lh agentReagents/debs/
ls -lh agentReagents/images/

# Search manifest
grep -i "rustdesk" agentReagents/docs/MANIFEST.md
```

**Using artifacts:**
```bash
# Copy to project
cp agentReagents/images/ubuntu-22.04-cloud.img /var/lib/libvirt/images/

# Install package
sudo dpkg -i agentReagents/debs/remote-desktop/rustdesk-*.deb
```

---

## 📦 Categories

### bins/
Store compiled executables, standalone binaries.

**Examples:**
- `benchscale` - VM orchestration tool
- `ion-deploy` - Deployment utilities
- Custom compiled tools

### tars/
Compressed source archives, release tarballs.

**Examples:**
- `ionChannel-v0.1.0.tar.gz` - Project releases
- `cosmic-comp-src.tar.xz` - Compositor sources

### debs/
Debian packages for Ubuntu/Pop!_OS installations.

**Subcategories:**
- `remote-desktop/` - RustDesk, NoMachine, etc.
- `development/` - Build tools, compilers
- `desktop/` - COSMIC, Wayland components

### isos/
Operating system installation images.

**Examples:**
- `pop-os_24.04_amd64_nvidia_22.iso`
- `ubuntu-22.04-live-server-amd64.iso`

### images/
Virtual machine disk images.

**Subcategories:**
- `cloud/` - Cloud-init enabled images
- `base/` - Base OS images
- `templates/` - Pre-configured templates

### configs/
Configuration files, templates, presets.

**Examples:**
- `cloud-init-templates/` - User-data, meta-data
- `ssh-configs/` - SSH configuration templates
- `benchscale-labs/` - Lab topology definitions

### scripts/
Utility scripts for common tasks.

**Examples:**
- `download-cloud-images.sh` - Fetch Ubuntu cloud images
- `setup-vm.sh` - VM initialization scripts
- `install-deps.sh` - Dependency installers

### docs/
Documentation, manifests, metadata.

**Files:**
- `MANIFEST.md` - Inventory of all stored artifacts
- `SOURCES.md` - URLs and sources for artifacts
- `CHECKSUMS.md` - SHA256 checksums for verification

---

## 🔒 Best Practices

### For Storage
1. **Use subdirectories** - Organize by purpose/category
2. **Document everything** - Update MANIFEST.md when adding files
3. **Include metadata** - Version, date, source URL
4. **Verify checksums** - Store SHA256 hashes in CHECKSUMS.md

### For Retrieval
1. **Check manifest first** - See what's available
2. **Verify checksums** - Ensure integrity
3. **Use symlinks** - Don't duplicate, link to agentReagents
4. **Report issues** - Update docs if something is missing/broken

### For Cleanup
1. **Remove old versions** - Keep only latest + previous
2. **Archive if needed** - Move to `../archive/agentReagents/`
3. **Update manifests** - Remove entries for deleted files

---

## 🤖 Autonomous Operations

### AI Agent Guidelines

**When downloading:**
```bash
# Always specify target directory
wget -P agentReagents/images/ <url>

# Add to manifest automatically
echo "$(basename $file) | $(date) | $url" >> agentReagents/docs/MANIFEST.md

# Generate checksum
sha256sum $file >> agentReagents/docs/CHECKSUMS.md
```

**When organizing:**
```bash
# Create subcategories as needed
mkdir -p agentReagents/debs/remote-desktop

# Use descriptive names
mv package.deb agentReagents/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb
```

**When cleaning:**
```bash
# Keep previous version as backup
mv old-version.deb agentReagents/debs/.archive/

# Document removal
echo "Removed: old-version.deb | Reason: Superseded by new-version.deb" >> agentReagents/docs/CHANGELOG.md
```

---

## 📊 Integration with Projects

### ionChannel
```bash
# Use cloud images from agentReagents
ln -s ../../agentReagents/images/cloud/ubuntu-22.04.img /var/lib/libvirt/images/

# Install dependencies
./agentReagents/scripts/install-deps.sh ionChannel
```

### benchScale
```bash
# Use base images
export BENCHSCALE_BASE_IMAGE_PATH="$(pwd)/../agentReagents/images/base"

# Load lab configs
cp agentReagents/configs/benchscale-labs/*.toml benchScale/examples/labs/
```

### Future Projects
- All syntheticChemistry projects can reference `../agentReagents/`
- Shared resources, no duplication
- Consistent organization across ecosystem

---

## 🎓 LMS-Style Organization

This directory follows Learning Management System principles:

1. **Categorization** - Clear folder structure
2. **Discoverability** - Easy to find what you need
3. **Documentation** - Every artifact documented
4. **Versioning** - Track what's current vs archived
5. **Access Control** - Central location, predictable paths
6. **Maintenance** - Regular cleanup, documented changes

---

## 📝 Quick Reference

```bash
# Download Ubuntu cloud image
wget -P agentReagents/images/cloud/ \
  https://cloud-images.ubuntu.com/releases/22.04/release/ubuntu-22.04-server-cloudimg-amd64.img

# Download RustDesk
wget -P agentReagents/debs/remote-desktop/ \
  https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb

# List all images
ls -lh agentReagents/images/*/

# Find a package
find agentReagents/ -name "*rustdesk*"

# Check manifest
cat agentReagents/docs/MANIFEST.md
```

---

**Created:** December 27, 2025  
**Maintained By:** AI agents + Human users  
**Used By:** All syntheticChemistry projects

