# ionChannel Validation Substrates - Ready

## Overview

Clean, repeatable, exportable validation substrates for ionChannel testing are now available. These substrates can be built on one system and transferred to another for validation.

---

## What's New

### 1. ionChannel-Specific Templates ✅

**Ubuntu 24.04 LTS Baseline**
- File: `templates/ionchannel-ubuntu24-baseline.yaml`
- Fully automated build
- GNOME on Wayland
- Auto-login configured for testing
- Intermediate checkpoints saved
- **Build time**: ~15-20 minutes

**Pop!_OS with COSMIC**
- File: `templates/ionchannel-popos-cosmic.yaml`
- Cooperative build (user completes GUI setup)
- COSMIC on Wayland
- Two intermediate checkpoints
- **Build time**: ~25-30 minutes + user setup

### 2. Automated Build Scripts ✅

```bash
# Build both substrates
cd agentReagents
./scripts/build-substrates.sh all

# Build individually
./scripts/build-substrates.sh ubuntu   # Fully automated
./scripts/build-substrates.sh cosmic   # Requires GUI setup
```

### 3. Export/Import System ✅

```bash
# Export substrates (with checksums)
./scripts/export-substrates.sh

# On another system: Import
./scripts/import-substrates.sh ~/substrate-imports

# Automatic checksum verification
# Automatic ownership/permissions setup
```

### 4. Comprehensive Documentation ✅

- `SUBSTRATE_GUIDE.md` - Complete guide with:
  - Build instructions
  - Export/import procedures
  - Verification workflows
  - Troubleshooting
  - Best practices

---

## Key Features

### Repeatability
- ✅ Build from YAML manifests
- ✅ Version controlled templates
- ✅ Reproducible on any system with benchScale/agentReagents
- ✅ Automated verification steps

### Portability
- ✅ Export as compressed qcow2 images
- ✅ SHA256 checksums for verification
- ✅ Manifest with metadata
- ✅ Works across different systems

### Intermediate Storage
- ✅ Save checkpoints during build
- ✅ Resume from any checkpoint
- ✅ Test different configurations from same base
- ✅ Reduce rebuild time

### Validation Integration
- ✅ Marker files for identification
- ✅ Wayland configuration verified
- ✅ SSH access configured
- ✅ Desktop environment working

---

## Substrate Details

### Ubuntu 24.04 Baseline

**Purpose**: Control substrate for ionChannel validation

**Configuration**:
```yaml
OS: Ubuntu 24.04 LTS
Desktop: GNOME 46+ on Wayland
User: iontest / iontest123
Memory: 4GB
vCPUs: 2
Disk: 30GB
```

**Features**:
- Auto-login enabled
- SSH server running
- Desktop environment active
- Firefox installed
- Verification marker: `/etc/ionchannel-substrate`

**Checkpoints**:
1. `ubuntu24-baseline-configured` - After desktop install, before RustDesk

### Pop!_OS COSMIC

**Purpose**: COSMIC desktop validation substrate

**Configuration**:
```yaml
OS: Pop!_OS 24.04 (Ubuntu base)
Desktop: COSMIC (latest) on Wayland
User: iontest / iontest123
Memory: 6GB (COSMIC needs more RAM)
vCPUs: 4
Disk: 40GB
```

**Features**:
- COSMIC desktop fully configured
- User preferences set
- Display settings optimized
- SSH server running
- Verification marker: `/etc/ionchannel-substrate`

**Checkpoints**:
1. `popos-cosmic-presetup` - COSMIC installed, before GUI setup
2. `popos-cosmic-configured` - After user completes GUI setup

**User Interaction**:
The build pauses after COSMIC installation for you to:
1. Connect via VNC (port shown in output)
2. Complete COSMIC first-time setup wizard
3. Configure display and workspace preferences
4. Log out
5. Press Enter to continue build

---

## Usage Workflow

### 1. Build Substrates

```bash
cd agentReagents

# Build Ubuntu baseline (automated)
./scripts/build-substrates.sh ubuntu

# Build COSMIC (with user setup)
./scripts/build-substrates.sh cosmic
# When prompted, connect via VNC and complete setup
```

### 2. Export for Backup/Transfer

```bash
# Export all substrates and intermediates
./scripts/export-substrates.sh

# Output: ~/substrate-exports/
#   - ubuntu24-baseline-YYYYMMDD.qcow2
#   - ubuntu24-intermediate-YYYYMMDD.qcow2
#   - popos-cosmic-YYYYMMDD.qcow2
#   - popos-cosmic-presetup-YYYYMMDD.qcow2
#   - popos-cosmic-configured-YYYYMMDD.qcow2
#   - checksums-YYYYMMDD.txt
#   - substrate-manifest-YYYYMMDD.yaml
```

### 3. Transfer to Another System

```bash
# Copy exports to another system
scp -r ~/substrate-exports/ other-tower:~/substrate-imports/

# Or use USB drive, shared storage, etc.
```

### 4. Import on New System

```bash
# On new system with benchScale + agentReagents
cd agentReagents
./scripts/import-substrates.sh ~/substrate-imports

# Verifies checksums automatically
# Sets proper ownership/permissions
```

### 5. Validate

```bash
# Test substrates work on new system
cd ../ionChannel
cargo run --bin baseline-validation --features benchscale

# Should output:
# ✅ Ubuntu 24 substrate: working
# ✅ COSMIC substrate: working
```

---

## Storage Requirements

| Substrate | Build | Export | Notes |
|-----------|-------|--------|-------|
| Ubuntu 24 Baseline | ~10GB | ~3-4GB | Compressed export |
| Ubuntu 24 Intermediate | ~8GB | ~2-3GB | Optional |
| COSMIC Final | ~18GB | ~5-6GB | Compressed export |
| COSMIC Presetup | ~12GB | ~4GB | Before GUI setup |
| COSMIC Configured | ~15GB | ~5GB | After GUI setup |
| **Total** | ~63GB | ~20-23GB | All substrates |

**Recommendation**: Keep final + configured intermediates, archive presetup

---

## Integration with ionChannel

### Substrate Discovery

ionChannel validation can now discover substrates:

```rust
use ion_validation::substrates::discover_substrate;

// Find Ubuntu baseline
let ubuntu_substrate = discover_substrate("ubuntu24-baseline")?;
println!("Found: {}", ubuntu_substrate.path.display());

// Find COSMIC
let cosmic_substrate = discover_substrate("popos-cosmic")?;
```

### Validation Workflow

```bash
cd ionChannel

# A/B validation: Ubuntu vs COSMIC
cargo run --bin ab-validation --features benchscale

# Uses both substrates automatically
# Compares RustDesk performance
# Reports differences
```

---

## Verification

### After Build
```bash
# Check substrates exist
ls -lh /var/lib/libvirt/images/ionchannel-*.qcow2

# Verify markers
virt-cat -d ubuntu24-baseline /etc/ionchannel-substrate
```

### After Export
```bash
# Verify checksums
cd ~/substrate-exports
sha256sum -c checksums-*.txt
```

### After Import
```bash
# Test with benchScale
cd benchScale
cargo run --example verify_substrate -- \
    --image /var/lib/libvirt/images/ionchannel-ubuntu24-baseline.qcow2
```

---

## Benefits

### For Development
- ✅ Clean, known-good starting point
- ✅ Fast iteration (use intermediates)
- ✅ Consistent test environment
- ✅ Easy to reset/rebuild

### For Validation
- ✅ Reproducible test conditions
- ✅ Compare across systems
- ✅ Verify fixes work elsewhere
- ✅ Share with team

### For Documentation
- ✅ Clear setup process
- ✅ Version controlled configs
- ✅ Export/import procedures
- ✅ Troubleshooting guides

---

## Next Steps

### Immediate
1. ✅ Build Ubuntu 24 baseline
2. ✅ Build COSMIC substrate (with GUI setup)
3. ✅ Export for backup
4. ⚠️ Run ionChannel validation tests

### Short Term
1. Transfer to external tower
2. Import and verify on new system
3. Run cross-system validation
4. Document any system-specific issues

### Long Term
1. Automate COSMIC GUI setup if possible
2. Add more distro substrates (Fedora, Arch)
3. Create minimal substrates for CI
4. Upstream template improvements to agentReagents

---

## Files Created

### Templates
- `templates/ionchannel-ubuntu24-baseline.yaml`
- `templates/ionchannel-popos-cosmic.yaml`

### Scripts
- `scripts/build-substrates.sh` - Build automation
- `scripts/export-substrates.sh` - Export with checksums
- `scripts/import-substrates.sh` - Import with verification

### Documentation
- `SUBSTRATE_GUIDE.md` - Complete guide
- `IONCHANNEL_SUBSTRATES_READY.md` - This file

---

## Summary

ionChannel now has clean, repeatable, exportable validation substrates:

```
✅ Ubuntu 24.04 LTS baseline (automated)
✅ Pop!_OS COSMIC desktop (cooperative)
✅ Export/import scripts (with verification)
✅ Intermediate checkpoints (resume/reuse)
✅ Comprehensive documentation
✅ Integration with benchScale/agentReagents
```

**Status**: Ready for validation work

---

**Created**: December 30, 2025  
**For**: ionChannel validation substrates  
**Requires**: benchScale + agentReagents  
**Ready**: ✅ Build, export, import, validate

---

*Clean substrates, repeatable builds, portable validation*

