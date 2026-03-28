# agentReagents Setup Guide

This repository contains scripts, configs, and automation for the ecoPrimals infrastructure supply chain: VM templates, shared ISOs and cloud images, and validation hooks for benchScale and primalSpring. All large binaries (ISOs, images) are downloaded via automated scripts.

## Quick Start (New Tower Setup)

```bash
# 1. Clone the repository
git clone <repo-url> agentReagents
cd agentReagents

# 2. Run automated setup (downloads everything)
bash scripts/setup-reagents.sh

# 3. Verify setup
bash scripts/verify-setup.sh
```

That's it! The setup script will:
- Download all ISOs (~13GB)
- Download cloud images (~2GB)
- Download RustDesk packages (~18MB)
- Set up directory structure
- Verify checksums
- Ready for template building

## What's In This Repo

### 📜 Scripts (Git Tracked)
- `scripts/setup-reagents.sh` - One-command setup for new towers
- `scripts/download-isos.sh` - Download all OS ISOs
- `scripts/download-cloud-images.sh` - Download Ubuntu cloud images
- `scripts/build-*.sh` - Template building automation
- `scripts/verify-setup.sh` - Verify everything is ready

### 📚 Documentation (Git Tracked)
- `README.md` - Overview and architecture
- `SETUP.md` - This file
- `ISO_DOWNLOAD_LINKS.md` - Direct links to all ISOs
- `MULTI_DISTRO_STRATEGY.md` - Multi-distro validation approach
- `docs/` - Additional documentation

### ⚙️ Configs (Git Tracked)
- `configs/` - Cloud-init templates and configs

### 💿 Binaries (Downloaded, Not in Git)
- `isos/` - OS installation ISOs (~13GB)
- `images/` - VM templates and cloud images (~13GB)
- `debs/` - Package files (~18MB)
- `bins/` - Additional binaries

## Manual Setup (If Needed)

### 1. Download ISOs

```bash
cd agentReagents
bash scripts/download-isos.sh
```

This downloads:
- Pop!_OS 22.04 LTS (~3.4GB)
- Pop!_OS 24.04 LTS (~3.4GB)
- Ubuntu 24.04 LTS (~6GB)

### 2. Download Cloud Images

```bash
bash scripts/download-cloud-images.sh
```

This downloads:
- Ubuntu 24.04 Server Cloud Image (~700MB)
- Ubuntu 22.04 Server Cloud Image (~600MB)

### 3. Download Packages

```bash
bash scripts/download-packages.sh
```

This downloads:
- RustDesk 1.2.3 x86_64 (~18MB)

### 4. Build Templates (Optional)

Build VM templates for faster testing:

```bash
# Ubuntu cloud image → COSMIC desktop + RustDesk (active golden-path builder)
sudo bash scripts/build-cosmic-cloud-automated.sh

# Ubuntu 22 + RustDesk (legacy; Pop!_OS–era pipeline — see scripts/legacy/)
sudo bash scripts/legacy/build-rustdesk-template.sh
```

## Directory Structure

```
agentReagents/
├── scripts/               # Build & setup automation (git tracked)
│   └── legacy/            # Pop!_OS / older template builders (reference)
├── configs/               # Cloud-init configs (git tracked)
├── docs/                  # Documentation (git tracked)
├── isos/                  # OS ISOs (downloaded, ~13GB)
├── images/                # VM templates (built/downloaded, ~13GB)
│   ├── base/             # Base OS images
│   ├── cloud/            # Cloud images
│   ├── intermediates/    # Build artifacts
│   └── templates/        # Ready-to-use templates
├── debs/                  # Package files (downloaded)
├── bins/                  # Additional binaries (downloaded)
└── tars/                  # Compressed archives

Total Size After Setup: ~26GB (mostly ISOs and templates)
Git Repo Size: <1MB (scripts and docs only)
```

## For AI Agents

This repository is designed to be agent-friendly:

1. **Self-Contained**: All resources are downloadable via scripts
2. **Reproducible**: Same steps work on any tower
3. **Automated**: Single command setup (`setup-reagents.sh`)
4. **Documented**: Every component has clear documentation
5. **Verifiable**: Checksums and verification scripts included

### Agent Instructions

To set up agentReagents on a new system:

```bash
# Clone repository
git clone <repo-url> ~/agentReagents
cd ~/agentReagents

# Run setup (downloads all binaries)
bash scripts/setup-reagents.sh

# Verify setup completed successfully
bash scripts/verify-setup.sh

# Proceed with template building or validation
```

## Integration with benchScale and primalSpring

Once agentReagents is set up, point benchScale at the shared cloud images and create a lab:

```bash
# From infra: agentReagents and benchScale are siblings under ecoPrimals/infra/
cd ~/Development/ecoPrimals/infra/benchScale
export BENCHSCALE_BASE_IMAGE_PATH="$(pwd)/../agentReagents/images/cloud"
./scripts/create-lab.sh --topology ecoprimals-tower-2node --name validation-lab --hypervisor qemu
```

For primalSpring Tier-2 checks (auto-discovers plasmidBin and benchScale):

```bash
cd ~/Development/ecoPrimals/springs/primalSpring
./scripts/validate_local_lab.sh --topology ecoprimals-tower-2node
```

## Updating agentReagents

When scripts or docs are updated:

```bash
cd agentReagents
git pull
# Re-run setup if needed
bash scripts/setup-reagents.sh --update
```

## Troubleshooting

### Missing ISOs
```bash
bash scripts/download-isos.sh
```

### Missing Cloud Images
```bash
bash scripts/download-cloud-images.sh
```

### Verify Everything
```bash
bash scripts/verify-setup.sh
```

### Clean and Rebuild
```bash
# Remove all downloaded binaries
rm -rf isos/*.iso images/**/*.qcow2 images/**/*.img

# Re-download everything
bash scripts/setup-reagents.sh
```

## Contributing

When adding new resources:
1. Add download links to `ISO_DOWNLOAD_LINKS.md` or similar
2. Create/update download scripts in `scripts/`
3. Update `.gitignore` if needed (don't commit large binaries)
4. Document in `README.md` and this file
5. Test on a fresh clone

## Links

- [ISO Download Links](ISO_DOWNLOAD_LINKS.md) - Direct URLs for all ISOs
- [Multi-Distro Strategy](MULTI_DISTRO_STRATEGY.md) - Validation approach
- [benchScale](../benchScale/) - Lab orchestration that consumes agentReagents images and configs

