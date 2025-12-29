# 🎯 Pop!_OS + COSMIC Template Creation Guide

## ✅ Resources Available

You have everything needed in `agentReagents`:
- ✅ **Pop!_OS ISO** (3GB) - `isos/pop-os_22.04_amd64_nvidia_22.iso`
- ✅ **RustDesk .deb** - `debs/remote-desktop/rustdesk-1.2.3-x86_64.deb`
- ✅ **Build scripts** - Ready to use

## 🚀 Three-Step Process

### Step 1: Create VM from ISO (~5 minutes + 20 min installation)

```bash
cd agentReagents
./scripts/build-popos-from-iso.sh
```

This will:
- Create a new VM with 25GB disk
- Attach the Pop!_OS ISO
- Open virt-viewer for you to complete installation

**During Installation:**
- Username: `iontest`
- Password: `iontest123`
- Hostname: `popos-template`
- Select "Clean Install"
- Wait ~20 minutes for installation

### Step 2: Configure Template (~5 minutes)

After installation completes and you're logged into the desktop:

```bash
cd agentReagents
./scripts/configure-popos-template.sh
```

This will:
- Copy RustDesk .deb to the VM
- Install and configure RustDesk
- Set up auto-start
- Enable SSH

### Step 3: Finalize Template (~2 minutes)

Shutdown the VM from inside:
```bash
sudo shutdown -h now
```

Then finalize:
```bash
cd agentReagents
./scripts/finalize-popos-template.sh
```

This will:
- Compress and optimize the image
- Save to `/var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2`
- Copy to `agentReagents/images/templates/`
- Save intermediate snapshot
- Clean up

## ✅ Result

**Pop!_OS + COSMIC + RustDesk Template:**
- Size: ~6-8GB compressed
- OS: Pop!_OS 22.04 with NVIDIA drivers
- Desktop: COSMIC (Wayland-native compositor)
- RDP: RustDesk 1.2.3 (auto-start)
- User: iontest / iontest123
- SSH: Enabled

## 🎯 Then Use It!

Once the template is created, it will automatically be used by:

```bash
cd ../ionChannel

# Single VM test
cargo run --bin autonomous-rustdesk-benchscale --features benchscale

# A/B validation (Control vs ionChannel)
cargo run --bin ab-validation --features benchscale
```

Both binaries are already configured to use the Pop!_OS template!

## ⏱️ Total Time

- **Step 1:** 5 min setup + 20 min Pop!_OS installation = 25 min
- **Step 2:** 5 min configuration
- **Step 3:** 2 min finalization
- **Total:** ~32 minutes hands-on

## 🎓 Why This Matters

**COSMIC is the actual blocker we're solving:**
- Modern Wayland-only compositor
- Restrictive security model (like the Reddit post described)
- Breaks automation tools
- Lacks granular capability control

**ionChannel adds:**
- Granular capability permissions
- Runtime extensibility
- User control over security
- No breaking of legitimate use cases

This is the **real validation** - not Ubuntu with GNOME, but Pop!_OS with COSMIC!

## 🚀 Ready to Start?

Run the first script:
```bash
cd agentReagents
./scripts/build-popos-from-iso.sh
```

Follow the virt-viewer prompts to install Pop!_OS, then continue with steps 2 and 3!

