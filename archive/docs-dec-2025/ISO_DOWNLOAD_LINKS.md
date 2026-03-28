# ISO Download Links

> **Legacy Document** — This guide references the syntheticChemistry/ionChannel era.
> For current ecoPrimals validation, see `specs/AGENTREAGENTS_EVOLUTION.md`.

Download these ISOs on your target tower before using the agentReagents archive.

## Pop!_OS ISOs

### Pop!_OS 22.04 LTS (with NVIDIA drivers)
- **Filename:** `pop-os_22.04_amd64_nvidia_22.iso`
- **Size:** ~3.4GB
- **Download:** https://iso.pop-os.org/22.04/amd64/nvidia/22/pop-os_22.04_amd64_nvidia_22.iso
- **Mirror:** https://pop-iso.sfo2.cdn.digitaloceanspaces.com/22.04/amd64/nvidia/22/pop-os_22.04_amd64_nvidia_22.iso
- **SHA256:** Available at https://pop.system76.com

### Pop!_OS 24.04 LTS (with NVIDIA drivers)
- **Filename:** `pop-os_24.04_amd64_nvidia_22.iso`
- **Size:** ~3.4GB
- **Download:** https://iso.pop-os.org/24.04/amd64/nvidia/22/pop-os_24.04_amd64_nvidia_22.iso
- **Mirror:** https://pop-iso.sfo2.cdn.digitaloceanspaces.com/24.04/amd64/nvidia/22/pop-os_24.04_amd64_nvidia_22.iso
- **SHA256:** Available at https://pop.system76.com

## Ubuntu ISOs

### Ubuntu 24.04 LTS (Noble Numbat) Desktop
- **Filename:** `ubuntu-24.04.3-desktop-amd64.iso`
- **Size:** ~6.0GB
- **Download:** https://releases.ubuntu.com/noble/ubuntu-24.04.3-desktop-amd64.iso
- **Torrent:** https://releases.ubuntu.com/noble/ubuntu-24.04.3-desktop-amd64.iso.torrent
- **SHA256:** Available at https://releases.ubuntu.com/noble/SHA256SUMS

### Ubuntu 24.04 LTS Server Cloud Image
- **Filename:** `ubuntu-24.04-server-cloudimg-amd64.img`
- **Size:** ~700MB
- **Download:** https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img
- **SHA256:** https://cloud-images.ubuntu.com/noble/current/SHA256SUMS

### Ubuntu 22.04 LTS Server Cloud Image
- **Filename:** `ubuntu-22.04-server-cloudimg-amd64.img`
- **Size:** ~600MB
- **Download:** https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img
- **SHA256:** https://cloud-images.ubuntu.com/jammy/current/SHA256SUMS

## Quick Download Script

Place ISOs in `agentReagents/isos/` directory:

```bash
#!/bin/bash
mkdir -p ~/agentReagents/isos
cd ~/agentReagents/isos

echo "📥 Downloading Pop!_OS 22.04..."
wget -c https://iso.pop-os.org/22.04/amd64/nvidia/22/pop-os_22.04_amd64_nvidia_22.iso

echo "📥 Downloading Pop!_OS 24.04..."
wget -c https://iso.pop-os.org/24.04/amd64/nvidia/22/pop-os_24.04_amd64_nvidia_22.iso

echo "📥 Downloading Ubuntu 24.04..."
wget -c https://releases.ubuntu.com/noble/ubuntu-24.04.3-desktop-amd64.iso

echo "✅ All ISOs downloaded!"
ls -lh *.iso
```

## Cloud Images

Place cloud images in `agentReagents/images/cloud/` directory:

```bash
#!/bin/bash
mkdir -p ~/agentReagents/images/cloud
cd ~/agentReagents/images/cloud

echo "📥 Downloading Ubuntu 24.04 cloud image..."
wget -c https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img \
    -O ubuntu-24.04-server-cloudimg-amd64.img

echo "📥 Downloading Ubuntu 22.04 cloud image..."
wget -c https://cloud-images.ubuntu.com/jammy/current/jammy-server-cloudimg-amd64.img \
    -O ubuntu-22.04-server-cloudimg-amd64.img

echo "✅ All cloud images downloaded!"
ls -lh *.img
```

## After Downloading

Once ISOs are downloaded:
1. Extract the agentReagents archive from USB
2. Place ISOs in `agentReagents/isos/`
3. Place cloud images in `agentReagents/images/cloud/`
4. Run validation: `cd ../benchScale && ./scripts/create-lab.sh --topology ecoprimals-tower-2node --name my-lab --hypervisor qemu` (or from this repo: `cd ../../springs/primalSpring && ./scripts/validate_local_lab.sh --topology ecoprimals-tower-2node`)

