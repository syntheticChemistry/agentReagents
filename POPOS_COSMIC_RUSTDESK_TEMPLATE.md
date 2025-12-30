# Pop!_OS 24 + COSMIC + RustDesk Template

**Date:** December 29, 2025  
**Status:** ✅ PRODUCTION READY  
**Build Time:** ~2 minutes  
**Template Size:** 1.1GB

---

## Overview

Complete Pop!_OS 24 template with COSMIC desktop (Wayland) and RustDesk pre-installed for ionChannel validation and remote desktop testing.

---

## Template Details

### Location
```
/var/lib/libvirt/images/popos-24-cosmic-rustdesk-template.qcow2
```

### Size
- **Template:** 1.1GB (compressed)
- **Virtual:** 35GB (expands as needed)
- **Format:** QCOW2

### Included Software

**Operating System:**
- Pop!_OS 24 (Ubuntu 24.04 LTS base)
- Kernel: 6.8.0-90-generic

**Desktop Environment:**
- COSMIC Desktop (System76)
- cosmic-comp (Wayland compositor)
- cosmic-greeter (login manager)
- Full COSMIC suite (panel, launcher, settings, etc.)

**Screen Capture:**
- PipeWire
- Wireplumber  
- GStreamer PipeWire plugin

**Remote Desktop:**
- RustDesk 1.2.3
- Auto-starts on login
- Configured for Wayland

**System:**
- SSH server enabled
- Avahi daemon (mDNS)
- Build tools
- Development utilities

---

## Credentials

**Default User:**
```
Username: cosmic
Password: CosmicDesk2025!
```

**SSH Access:**
- Enabled by default
- Password authentication: Yes
- Key authentication: Yes (via cloud-init)

---

## Usage

### With benchScale (Recommended)

```rust
use benchscale::backend::LibvirtBackend;
use benchscale::CloudInit;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let backend = LibvirtBackend::new()?;
    
    // Optional: Customize with cloud-init
    let cloud_init = CloudInit::builder()
        .add_user("testuser", ssh_public_key)
        .build();
    
    // Create VM from template
    let vm = backend.create_from_template(
        "my-cosmic-desktop",
        &PathBuf::from("/var/lib/libvirt/images/popos-24-cosmic-rustdesk-template.qcow2"),
        Some(&cloud_init),
        4096,  // 4GB RAM (recommended)
        2,     // 2 vCPUs
        false  // no intermediate save
    ).await?;
    
    println!("VM created: {} at {}", vm.name, vm.ip_address);
    
    Ok(())
}
```

### With virt-install (Direct)

```bash
# Create VM from template
sudo virt-install \
    --name my-cosmic-vm \
    --memory 4096 \
    --vcpus 2 \
    --disk /var/lib/libvirt/images/my-cosmic-vm.qcow2,size=35,backing_store=/var/lib/libvirt/images/popos-24-cosmic-rustdesk-template.qcow2 \
    --os-variant ubuntu24.04 \
    --network network=default \
    --graphics vnc,listen=0.0.0.0 \
    --import \
    --noautoconsole
```

---

## Accessing the VM

### Via VNC (GUI Access)

```bash
# Get VNC display
sudo virsh vncdisplay my-cosmic-vm

# Connect with VNC viewer
vncviewer localhost:5900  # Adjust port as needed
```

### Via SSH

```bash
# Get IP address
sudo virsh domifaddr my-cosmic-vm

# Connect
ssh cosmic@<ip-address>
# Password: CosmicDesk2025!
```

### Via RustDesk

1. **Access VM via VNC first**
2. **Login** (should auto-login as 'cosmic')
3. **Wait for RustDesk to auto-start**
4. **Note the RustDesk ID and password** shown in window
5. **Connect from remote computer** using that ID

---

## Testing RustDesk

### Quick Test

```bash
# Create VM
cd ionChannel
cargo run --example create_cosmic_vm --features benchscale

# Access via VNC to get RustDesk ID
vncviewer localhost:5900

# From remote computer:
# 1. Install RustDesk
# 2. Enter the VM's RustDesk ID
# 3. Enter password
# 4. Should connect to COSMIC desktop!
```

### Automated Extraction

```bash
# Create VM and extract RustDesk ID via SSH
ssh cosmic@<vm-ip> "cat ~/.config/RustDesk/RustDesk2.toml | grep -E '(^id|^password)'"
```

Note: RustDesk ID is only generated after first GUI login.

---

## Features for ionChannel Testing

### Wayland Support ✅
- COSMIC uses native Wayland
- cosmic-comp compositor
- XWayland for compatibility

### Screen Capture ✅
- PipeWire configured
- Portal support ready
- Screen sharing capable

### Remote Desktop ✅
- RustDesk pre-installed
- Auto-starts on login
- Configured for Wayland

### ionChannel Ready ✅
- Can install ionChannel portal
- Test capability-based access
- Validate Wayland remote desktop

---

## Rebuilding the Template

If you need to rebuild or customize:

```bash
cd agentReagents/scripts
sudo ./build-popos-24-cosmic-rustdesk.sh
```

**Build time:** ~20-40 minutes  
**Requirements:**
- Ubuntu 24.04 cloud image
- 35GB free disk space
- 4GB RAM for builder VM
- Internet connection

---

## Customization

### Modify the Build Script

Edit `agentReagents/scripts/build-popos-24-cosmic-rustdesk.sh`:

**Add packages:**
```yaml
packages:
  - your-package-here
```

**Add commands:**
```yaml
runcmd:
  - your-command-here
```

**Change credentials:**
```yaml
chpasswd:
  list: |
    cosmic:YourPasswordHere
```

---

## Comparison with Baseline

| Feature | Baseline | +RustDesk |
|---------|----------|-----------|
| COSMIC Desktop | ✅ | ✅ |
| Wayland | ✅ | ✅ |
| PipeWire | ✅ | ✅ |
| SSH | ✅ | ✅ |
| RustDesk | ❌ | ✅ |
| Auto-start RustDesk | ❌ | ✅ |
| Remote Desktop Ready | ⚠️ Manual | ✅ Automatic |
| Size | ~900MB | ~1.1GB |

---

## Troubleshooting

### RustDesk Not Starting

```bash
# SSH into VM
ssh cosmic@<ip>

# Check RustDesk status
systemctl --user status rustdesk

# Start manually
WAYLAND_DISPLAY=wayland-0 /usr/bin/rustdesk &

# Check logs
journalctl --user -u rustdesk
```

### COSMIC Not Loading

```bash
# Check display manager
sudo systemctl status cosmic-greeter

# Check session
echo $XDG_SESSION_TYPE  # Should be "wayland"
echo $WAYLAND_DISPLAY   # Should be set
```

### No RustDesk ID

RustDesk ID is only generated after first GUI login:
1. Access VM via VNC
2. Login to COSMIC desktop
3. Wait for RustDesk to start
4. ID will be generated

### VNC Not Working

```bash
# Verify VNC is enabled
sudo virsh dumpxml my-cosmic-vm | grep graphics

# Should show:
# <graphics type='vnc' port='5900' autoport='yes' listen='0.0.0.0'>
```

---

## Performance Recommendations

### Minimum Requirements
- **RAM:** 2GB (will be slow)
- **vCPUs:** 1 (will be slow)
- **Disk:** 20GB

### Recommended
- **RAM:** 4GB
- **vCPUs:** 2
- **Disk:** 35GB

### Optimal
- **RAM:** 8GB
- **vCPUs:** 4
- **Disk:** 50GB

---

## Integration with ionChannel

### Validation Workflow

1. **Create VM from template**
   ```bash
   cargo run --example create_cosmic_vm --features benchscale
   ```

2. **Access and get RustDesk ID**
   ```bash
   vncviewer localhost:5900
   # Note RustDesk ID
   ```

3. **Establish baseline** (without ionChannel)
   - Connect from remote computer
   - Test screen sharing
   - Note behavior

4. **Install ionChannel portal**
   ```bash
   ssh cosmic@<vm-ip>
   cd ionChannel
   cargo build --release
   # Install portal
   ```

5. **Test with ionChannel**
   - Test capability grants
   - Test selective access
   - Compare with baseline

6. **Validate improvements**
   - Performance
   - Security (capabilities)
   - Functionality

---

## Files

### Template
```
/var/lib/libvirt/images/popos-24-cosmic-rustdesk-template.qcow2
```

### Build Script
```
agentReagents/scripts/build-popos-24-cosmic-rustdesk.sh
```

### Build Log
```
/tmp/popos-cosmic-rustdesk-build.log
```

---

## Summary

**Status:** ✅ Production ready  
**Purpose:** ionChannel validation + Remote desktop testing  
**Desktop:** COSMIC (Wayland)  
**Remote:** RustDesk pre-installed  
**Size:** 1.1GB template → 35GB virtual  
**Boot time:** ~30 seconds  
**Ready to use:** Immediately  

**This template provides a complete, production-ready environment for testing ionChannel's Wayland remote desktop capabilities with RustDesk.** 🚀

