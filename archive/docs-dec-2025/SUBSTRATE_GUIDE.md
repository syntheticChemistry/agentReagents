# ionChannel Validation Substrate Guide

## Overview

This guide explains how to create, export, and rebuild clean validation substrates for ionChannel testing. These substrates are portable and can be rebuilt on any system with benchScale and agentReagents.

---

## Available Substrates

### 1. Ubuntu 24.04 LTS Baseline
- **Template**: `templates/ionchannel-ubuntu24-baseline.yaml`
- **Desktop**: GNOME on Wayland
- **Fully Automated**: ✅ Yes
- **User Interaction**: ❌ No
- **Build Time**: ~15-20 minutes

### 2. Pop!_OS COSMIC
- **Template**: `templates/ionchannel-popos-cosmic.yaml`
- **Desktop**: COSMIC on Wayland
- **Fully Automated**: ⚠️ Partial (requires GUI setup)
- **User Interaction**: ✅ Yes (one-time setup)
- **Build Time**: ~25-30 minutes + user setup time

---

## Building Substrates

### Prerequisites
```bash
# Ensure you have:
- benchScale with libvirt support
- agentReagents with templates
- libvirt running
- Base cloud images downloaded
```

### Ubuntu 24.04 Baseline (Fully Automated)

```bash
cd agentReagents

# Build with your SSH key
cargo run --release -- build \
    templates/ionchannel-ubuntu24-baseline.yaml \
    --ssh-key "$(cat ~/.ssh/id_rsa.pub)"

# Output will be in:
# /var/lib/libvirt/images/ionchannel-ubuntu24-baseline.qcow2

# Intermediate saved at:
# /var/lib/libvirt/images/ubuntu24-baseline-configured.qcow2
```

### Pop!_OS COSMIC (With User Setup)

```bash
cd agentReagents

# Start the build
cargo run --release -- build \
    templates/ionchannel-popos-cosmic.yaml \
    --ssh-key "$(cat ~/.ssh/id_rsa.pub)"

# When prompted for user verification:
# 1. Build will pause and display VNC port
# 2. Connect via VNC: vncviewer localhost:5900
# 3. Complete COSMIC first-time setup
# 4. Press Enter in terminal to continue

# Intermediates saved at:
# - popos-cosmic-presetup.qcow2 (before GUI setup)
# - popos-cosmic-configured.qcow2 (after GUI setup)
```

---

## Exporting Substrates

### Why Export?
- Share substrates across development systems
- Verify builds work on other hardware
- Backup known-good configurations
- Distribute to team members

### Export Process

```bash
# Export Ubuntu 24.04 baseline
cd /var/lib/libvirt/images

# Create export directory
mkdir -p ~/substrate-exports

# Export final image
sudo qemu-img convert -c -O qcow2 \
    ionchannel-ubuntu24-baseline.qcow2 \
    ~/substrate-exports/ionchannel-ubuntu24-baseline-$(date +%Y%m%d).qcow2

# Export intermediate (optional but recommended)
sudo qemu-img convert -c -O qcow2 \
    ubuntu24-baseline-configured.qcow2 \
    ~/substrate-exports/ubuntu24-intermediate-$(date +%Y%m%d).qcow2

# Generate checksums
cd ~/substrate-exports
sha256sum *.qcow2 > checksums.txt

# Create metadata
cat > substrate-manifest.yaml <<EOF
substrates:
  - name: ionchannel-ubuntu24-baseline
    version: 1.0.0
    date: $(date -Iseconds)
    os: ubuntu-24.04
    desktop: gnome-wayland
    size: $(du -h ionchannel-ubuntu24-baseline-*.qcow2 | cut -f1)
    checksum: $(sha256sum ionchannel-ubuntu24-baseline-*.qcow2 | cut -d' ' -f1)
    
  - name: ubuntu24-baseline-intermediate
    version: 1.0.0
    date: $(date -Iseconds)
    checkpoint: configured
    size: $(du -h ubuntu24-intermediate-*.qcow2 | cut -f1)
    checksum: $(sha256sum ubuntu24-intermediate-*.qcow2 | cut -d' ' -f1)
EOF
```

### Export COSMIC Substrate

```bash
# Export final COSMIC image
sudo qemu-img convert -c -O qcow2 \
    /var/lib/libvirt/images/ionchannel-popos-cosmic.qcow2 \
    ~/substrate-exports/ionchannel-popos-cosmic-$(date +%Y%m%d).qcow2

# Export both intermediates
sudo qemu-img convert -c -O qcow2 \
    /var/lib/libvirt/images/popos-cosmic-presetup.qcow2 \
    ~/substrate-exports/popos-cosmic-presetup-$(date +%Y%m%d).qcow2

sudo qemu-img convert -c -O qcow2 \
    /var/lib/libvirt/images/popos-cosmic-configured.qcow2 \
    ~/substrate-exports/popos-cosmic-configured-$(date +%Y%m%d).qcow2

# Update checksums
cd ~/substrate-exports
sha256sum *.qcow2 >> checksums.txt
```

---

## Importing Substrates (On Another System)

### Prerequisites on New System
```bash
# Install benchScale and agentReagents
git clone git@github.com:syntheticChemistry/benchScale.git
git clone git@github.com:syntheticChemistry/agentReagents.git

cd benchScale && cargo build --release
cd ../agentReagents && cargo build --release
```

### Import Process

```bash
# Transfer substrates to new system
# (Use scp, USB drive, shared storage, etc.)

# On new system:
cd ~/substrate-imports  # Where you copied the files

# Verify checksums
sha256sum -c checksums.txt

# Import to libvirt
sudo cp ionchannel-ubuntu24-baseline-*.qcow2 \
    /var/lib/libvirt/images/ionchannel-ubuntu24-baseline.qcow2

sudo cp ubuntu24-intermediate-*.qcow2 \
    /var/lib/libvirt/images/ubuntu24-baseline-configured.qcow2

# Set ownership
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/ionchannel-*.qcow2
sudo chmod 644 /var/lib/libvirt/images/ionchannel-*.qcow2
```

### Verification Test

```bash
# Test substrate with benchScale
cd benchScale

# Create test VM from imported substrate
cargo run --example test_substrate -- \
    --image /var/lib/libvirt/images/ionchannel-ubuntu24-baseline.qcow2 \
    --verify

# Should output:
# ✅ Substrate marker found: ubuntu24-baseline
# ✅ Wayland: gnome
# ✅ SSH accessible
# ✅ Desktop environment: working
```

---

## Rebuilding from Scratch

### Why Rebuild?
- Verify substrate is reproducible
- Update base packages to latest
- Modify configuration
- Test on different architecture

### Rebuild Process

```bash
# On new system with benchScale + agentReagents:

# 1. Download base cloud image
cd agentReagents
./scripts/download-cloud-images.sh

# 2. Rebuild Ubuntu baseline
cargo run --release -- build \
    templates/ionchannel-ubuntu24-baseline.yaml \
    --ssh-key "$(cat ~/.ssh/id_rsa.pub)"

# 3. Compare with imported substrate
sha256sum /var/lib/libvirt/images/ionchannel-ubuntu24-baseline.qcow2
# Note: Won't match exactly due to timestamps, but validation should pass

# 4. Run validation tests
cd ../ionChannel
cargo run --bin baseline-validation --features benchscale
```

---

## Intermediate Checkpoints

### Purpose
- Save state before user interaction
- Allow resuming from specific points
- Test different configurations from same base
- Reduce rebuild time

### Using Intermediates

```bash
# Start from intermediate instead of base image
# Edit template to use intermediate as base_image:

name: custom-cosmic-test
base_image: /var/lib/libvirt/images/popos-cosmic-configured.qcow2
# ... rest of config

# Or use benchScale directly:
cd benchScale
cargo run --example create_from_template -- \
    --base-image /var/lib/libvirt/images/popos-cosmic-configured.qcow2 \
    --name cosmic-test-1
```

### Intermediate Locations

Default intermediate storage:
```
/var/lib/libvirt/images/
├── ionchannel-ubuntu24-baseline.qcow2      # Final
├── ubuntu24-baseline-configured.qcow2       # Intermediate
├── ionchannel-popos-cosmic.qcow2           # Final
├── popos-cosmic-presetup.qcow2             # Intermediate (pre-GUI)
└── popos-cosmic-configured.qcow2           # Intermediate (post-GUI)
```

---

## Storage Management

### Disk Space Requirements

| Substrate | Base | Intermediate(s) | Final | Total |
|-----------|------|----------------|-------|-------|
| Ubuntu 24 | 3GB | 8GB | 10GB | ~21GB |
| COSMIC | 3GB | 12GB + 15GB | 18GB | ~48GB |

### Cleanup

```bash
# Remove build intermediates (keep final only)
sudo rm /var/lib/libvirt/images/*-intermediate-*.qcow2
sudo rm /var/lib/libvirt/images/*-presetup.qcow2

# Archive to external storage
tar -czf substrates-archive-$(date +%Y%m%d).tar.gz \
    ~/substrate-exports/*.qcow2 \
    ~/substrate-exports/checksums.txt \
    ~/substrate-exports/substrate-manifest.yaml

# Move to external storage
mv substrates-archive-*.tar.gz /mnt/external-drive/
```

---

## Validation Workflow

### Complete Validation Cycle

```bash
# 1. Build substrates
cd agentReagents
./build-all-substrates.sh  # Builds both Ubuntu and COSMIC

# 2. Export for backup
./export-substrates.sh

# 3. Run ionChannel validation
cd ../ionChannel
cargo run --bin ab-validation --features benchscale

# 4. Transfer to another system
scp ~/substrate-exports/*.qcow2 other-tower:~/imports/

# 5. On other system: Import and verify
./import-and-verify.sh
```

---

## Troubleshooting

### Build Fails
```bash
# Check logs
tail -f /var/log/libvirt/qemu/*.log

# Check VM is accessible
virsh list --all
virsh console <vm-name>

# Restart from intermediate
# (Edit template to use intermediate as base_image)
```

### Export Fails
```bash
# Check disk space
df -h /var/lib/libvirt/images

# Check permissions
ls -l /var/lib/libvirt/images

# Use sudo for qemu-img convert
```

### Import Fails
```bash
# Verify checksums first
sha256sum -c checksums.txt

# Check libvirt ownership
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/*.qcow2
```

---

## Best Practices

### 1. Version Control
- Tag substrate versions in manifest
- Include build date in filename
- Keep checksums with exports

### 2. Testing
- Always validate after build
- Test substrates on clean system
- Verify reproduced builds work

### 3. Storage
- Export immediately after successful build
- Keep intermediates until validated
- Archive to external storage
- Document any manual changes

### 4. Documentation
- Note any user interaction steps
- Document validation criteria
- Record known issues
- Maintain changelog

---

## Quick Reference

### Build Commands
```bash
# Ubuntu baseline
agentReagents/$ cargo run -- build templates/ionchannel-ubuntu24-baseline.yaml

# COSMIC (with GUI setup)
agentReagents/$ cargo run -- build templates/ionchannel-popos-cosmic.yaml
```

### Export Commands
```bash
# Quick export
sudo qemu-img convert -c -O qcow2 \
    /var/lib/libvirt/images/source.qcow2 \
    ~/exports/dest-$(date +%Y%m%d).qcow2
```

### Import Commands
```bash
# Quick import
sudo cp ~/imports/substrate.qcow2 /var/lib/libvirt/images/
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/substrate.qcow2
```

---

**Guide Version**: 1.0.0  
**Last Updated**: December 30, 2025  
**For**: ionChannel validation substrates  
**Requires**: benchScale + agentReagents

---

*Building clean, repeatable, portable validation substrates*

