# agentReagents Manifest

**Last Updated:** December 27, 2025

This file tracks all artifacts stored in agentReagents.

---

## Format

```
filename | category | description | date | source
```

---

## Current Inventory

### Images (images/)

*(No entries yet - add Ubuntu cloud images here)*

**Recommended:**
- ubuntu-22.04-server-cloudimg-amd64.img | Cloud-init enabled base image

### Packages (debs/)

*(No entries yet - add .deb packages here)*

**Recommended:**
- rustdesk-1.2.3-x86_64.deb | Remote desktop for testing

### ISOs (isos/)

*(No entries yet - add ISO images here)*

### Binaries (bins/)

*(No entries yet - add compiled binaries here)*

### Archives (tars/)

*(No entries yet - add compressed archives here)*

### Configurations (configs/)

*(No entries yet - add config templates here)*

### Scripts (scripts/)

*(No entries yet - add utility scripts here)*

---

## How to Update

When adding a file:
```bash
echo "filename.ext | category | description | $(date +%Y-%m-%d) | source-url" >> agentReagents/docs/MANIFEST.md
```

When removing a file:
```bash
# Move to archive first
mv agentReagents/category/file ../archive/agentReagents/

# Document in CHANGELOG.md
echo "Removed: file | $(date +%Y-%m-%d) | Reason" >> agentReagents/docs/CHANGELOG.md
```

---

## Quick Downloads

### Ubuntu Cloud Image
```bash
cd agentReagents/images/cloud
wget https://cloud-images.ubuntu.com/releases/22.04/release/ubuntu-22.04-server-cloudimg-amd64.img
echo "ubuntu-22.04-server-cloudimg-amd64.img | images/cloud | Ubuntu 22.04 cloud-init | $(date +%Y-%m-%d) | cloud-images.ubuntu.com" >> ../../docs/MANIFEST.md
```

### RustDesk
```bash
cd agentReagents/debs/remote-desktop
wget https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb
echo "rustdesk-1.2.3-x86_64.deb | debs/remote-desktop | RustDesk v1.2.3 | $(date +%Y-%m-%d) | github.com/rustdesk" >> ../../docs/MANIFEST.md
```

ubuntu-22.04-server-cloudimg-amd64.img | images/cloud | Ubuntu 22.04 cloud-init enabled | 2025-12-27 | cloud-images.ubuntu.com
rustdesk-1.2.3-x86_64.deb | debs/remote-desktop | RustDesk v1.2.3 remote desktop | 2025-12-27 | github.com/rustdesk
pop-os_22.04_amd64_nvidia_22.iso | isos | Pop!_OS 22.04 with NVIDIA drivers | 2025-12-27 | pop.system76.com
