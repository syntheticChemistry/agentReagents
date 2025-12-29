# Building Pop!_OS + COSMIC Template

The template builder script has started but requires sudo permissions for libvirt operations.

## Current Status

✅ Downloaded Ubuntu 22.04 cloud image (690MB) - will be customized with Pop!_OS packages  
⏳ Waiting for sudo permission to continue...

## What the Builder Will Do

1. **Create base disk** from Ubuntu cloud image
2. **Install Pop!_OS packages**:
   - `pop-desktop` - Pop!_OS desktop environment
   - `cosmic-session` - COSMIC session manager  
   - `cosmic-comp` - COSMIC Wayland compositor
3. **Install RustDesk** 1.2.3
4. **Configure Wayland** as default
5. **Set GDM** as display manager
6. **Auto-start RustDesk** on login
7. **Save as template** to agentReagents

## Estimated Time

**Total: 20-40 minutes**
- Package downloads: 10-15 minutes
- Installation: 10-20 minutes  
- Configuration: 2-5 minutes

## Alternative: Use Existing Template

If you already have a Pop!_OS VM with COSMIC and RustDesk, you can convert it:

```bash
# Stop the VM
virsh shutdown your-popos-vm

# Convert to template
sudo qemu-img convert -O qcow2 -c \
  /var/lib/libvirt/images/your-popos-vm.qcow2 \
  /var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2

# Make accessible
sudo chown libvirt-qemu:kvm /var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2

# Copy to agentReagents
cp /var/lib/libvirt/images/popos-cosmic-rustdesk-template.qcow2 \
   agentReagents/images/templates/
```

## Or: Continue with Builder

To continue the automated build, you'll need to allow the sudo commands in the script.

The builder is waiting at:
```
📋 Copying base image to libvirt directory...
```

Once you provide sudo access, it will complete automatically.

## What Gets Built

**Pop!_OS + COSMIC/Wayland + RustDesk Template:**
- Base: Ubuntu 22.04 (Pop!_OS repos added)
- Desktop: COSMIC (Wayland-native)
- Compositor: `cosmic-comp` (not X11)
- RDP: RustDesk 1.2.3
- User: `iontest` / `iontest123`
- Size: ~5-6GB

This is the **actual target environment** for ionChannel validation - COSMIC is the modern Wayland compositor that Pop!_OS uses, and it's where we need to test capability-based security.

