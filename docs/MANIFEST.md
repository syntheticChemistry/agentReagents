# agentReagents Manifest

**Last Updated:** March 28, 2026

This file tracks all artifacts stored in or downloadable by agentReagents.
Large artifacts (ISOs, images, debs) are git-ignored and populated by scripts.

---

## Current Inventory

### Cloud Images (`images/cloud/`)

| Artifact | Size | Source | Download Script |
|----------|------|--------|-----------------|
| `ubuntu-22.04-server-cloudimg-amd64.img` | ~620 MB | cloud-images.ubuntu.com | `download-cloud-images.sh` |
| `ubuntu-24.04-server-cloudimg-amd64.img` | ~650 MB | cloud-images.ubuntu.com | `download-cloud-images.sh` |

### Packages (`debs/`)

| Artifact | Size | Source | Download Script |
|----------|------|--------|-----------------|
| `debs/remote-desktop/rustdesk-1.2.3-x86_64.deb` | ~18 MB | github.com/rustdesk | `download-packages.sh` |

### ISOs (`isos/`)

| Artifact | Size | Source | Download Script |
|----------|------|--------|-----------------|
| `pop-os_22.04_amd64_nvidia_22.iso` | ~2.8 GB | pop.system76.com | `download-isos.sh` |
| `ubuntu-24.04.1-desktop-amd64.iso` | ~5.7 GB | releases.ubuntu.com | `download-isos.sh` |

### Configurations (`configs/`)

| Artifact | Tracked | Description |
|----------|---------|-------------|
| `ecoprimals-node.yaml` | git-tracked | Cloud-init config for ecoPrimals gate VMs |

### Binaries (`bins/`)

Empty — primal binaries live in `plasmidBin`, not here.

### Templates (`images/templates/`)

Built locally by template build scripts, not downloaded. `.qcow2` files.

### Archives (`tars/`)

Empty — reserved for compressed artifact bundles.

---

## Verification

Checksums for downloaded artifacts are in `docs/CHECKSUMS.md`.
Run `scripts/verify-setup.sh` to check integrity.

---

## How to Update

When adding a downloaded artifact, update this file and `docs/CHECKSUMS.md`:

```bash
sha256sum path/to/artifact >> docs/CHECKSUMS.md
```
