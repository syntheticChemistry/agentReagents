# 🎯 Pop!_OS 24.04 + COSMIC Installation Guide

> **Legacy Document** — This guide references the syntheticChemistry/ionChannel era.
> For current ecoPrimals validation, see `specs/AGENTREAGENTS_EVOLUTION.md`.

## ✅ VM Created Successfully!

**VM Name:** `popos24-cosmic-template-builder`  
**Disk Size:** 30GB  
**Status:** Installing Pop!_OS 24.04

The virt-viewer window should now be open showing the Pop!_OS installer.

---

## 📋 Installation Steps (15-20 minutes)

### Step 1: Language & Keyboard
- Language: **English**
- Keyboard: **US** (or your preference)

### Step 2: Installation Type
- Select: **Clean Install**
- Drive: Default (30GB disk)

### Step 3: User Account ⭐ IMPORTANT
```
Username: iontest
Password: iontest123
Hostname: popos24-cosmic
```

### Step 4: Desktop Environment ⭐ CRITICAL
**SELECT COSMIC, NOT GNOME!**

Pop!_OS 24.04 may offer both:
- **COSMIC** ← Choose this! (Wayland-native, Rust-based)
- ~~GNOME~~ ← Don't choose this

**Why:** COSMIC is the cutting-edge Wayland compositor we're validating against!

### Step 5: Wait for Installation
- Installation: ~15-20 minutes
- Don't interrupt the process
- VM will prompt to reboot when done

### Step 6: First Boot
- Remove installation media (automatic)
- Reboot
- Login to COSMIC desktop
  - User: `iontest`
  - Password: `iontest123`

---

## 🚀 After Installation Complete

Once you're logged into the COSMIC desktop:

### Verify COSMIC is Running
```bash
# In terminal inside the VM:
echo $XDG_CURRENT_DESKTOP
# Should show: COSMIC or cosmic

# Check Wayland
echo $WAYLAND_DISPLAY
# Should show: wayland-0 or similar
```

### Run Configuration Script
From your host machine:

```bash
cd /path/to/agentReagents
./scripts/configure-popos-24-template.sh
```

This will:
- Copy RustDesk .deb to VM
- Install RustDesk
- Configure auto-start
- Set up SSH

### Finalize Template
After configuration succeeds:

1. **Shutdown VM** (from inside):
   ```bash
   sudo shutdown -h now
   ```

2. **Finalize template** (from host):
   ```bash
   cd agentReagents
   ./scripts/finalize-popos-24-template.sh
   ```

---

## ✅ Expected Result

**Final Template:**
- Location: `/var/lib/libvirt/images/popos-24.04-cosmic-rustdesk-template.qcow2`
- Size: ~5-7GB (compressed)
- Contents:
  - Pop!_OS 24.04 LTS
  - COSMIC desktop (Wayland)
  - RustDesk 1.2.3
  - User: iontest/iontest123

---

## 🎯 Then Use It!

Once template is built:

```bash
cd ../ionChannel

# Single VM test
cargo run --bin autonomous-rustdesk-benchscale --features benchscale

# A/B validation
cargo run --bin ab-validation --features benchscale

# Multi-distribution validation
cargo run --bin multi-distro-validation --features benchscale
```

---

## 🔧 Troubleshooting

**If virt-viewer didn't open:**
```bash
virt-viewer popos24-cosmic-template-builder
```

**If VM needs restart:**
```bash
sudo virsh reboot popos24-cosmic-template-builder
```

**Check VM status:**
```bash
sudo virsh domstate popos24-cosmic-template-builder
```

**Get console access:**
```bash
sudo virsh console popos24-cosmic-template-builder
```

---

## 🎓 Why COSMIC Matters

**COSMIC** = Computer Operating System Main Interface Components
- Written in **Rust** (like ionChannel!)
- **Wayland-native** (no X11 fallback)
- **Modern compositor** (iced-based rendering)
- **Restrictive security** (the problem we're solving)

**This is the exact environment** described in the Reddit post - where automation tools break, screen capture is restricted, and users lose control. **ionChannel is designed to fix this while maintaining security!**

---

**Installation is now running. Come back in ~20 minutes when it's ready for configuration!** ⏱️

