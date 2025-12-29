# agentReagents Sources

**Purpose:** Track source URLs for all downloaded artifacts.

---

## Ubuntu Cloud Images

**Base URL:** https://cloud-images.ubuntu.com/releases/

### Ubuntu 22.04 (Jammy)
```
https://cloud-images.ubuntu.com/releases/22.04/release/ubuntu-22.04-server-cloudimg-amd64.img
```

### Ubuntu 24.04 (Noble)
```
https://cloud-images.ubuntu.com/releases/24.04/release/ubuntu-24.04-server-cloudimg-amd64.img
```

---

## Remote Desktop Software

### RustDesk
**Base URL:** https://github.com/rustdesk/rustdesk/releases/

**Latest:**
```
https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb
```

### NoMachine (Alternative)
**Base URL:** https://www.nomachine.com/download/

---

## Pop!_OS

**Base URL:** https://pop.system76.com/

### Pop!_OS 22.04 LTS
```
https://iso.pop-os.org/22.04/amd64/nvidia/22/pop-os_22.04_amd64_nvidia_22.iso
https://iso.pop-os.org/22.04/amd64/intel/22/pop-os_22.04_amd64_intel_22.iso
```

---

## COSMIC Desktop

**Build from source:**
```
https://github.com/pop-os/cosmic-comp
https://github.com/pop-os/cosmic-settings
```

---

## Development Tools

### Rust
```
https://sh.rustup.rs
```

### Build Essentials
```
# Ubuntu/Debian
apt-get install build-essential
```

---

## Template for New Sources

```
## Category Name

**Base URL:** https://example.com/

### Package Name
Description

**Download:**
```
direct-download-url
```

**Checksums:** (if available)
```
checksum-url
```
```

---

**Maintained By:** AI agents (autonomous updates) + Humans (verification)

