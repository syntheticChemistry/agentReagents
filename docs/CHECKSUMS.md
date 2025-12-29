# agentReagents Checksums

**Purpose:** SHA256 checksums for verifying artifact integrity.

---

## Format

```
<sha256sum>  <filename>
```

---

## How to Use

### Verify a file
```bash
cd agentReagents
sha256sum -c docs/CHECKSUMS.md --ignore-missing
```

### Add a checksum
```bash
sha256sum path/to/file >> docs/CHECKSUMS.md
```

---

## Checksums

*(Empty - checksums will be added when artifacts are downloaded)*

---

**Note:** Always verify checksums before using downloaded artifacts!

2cbbe814d84c9dc7d749ea0afda924d9e26f771a3f824b48f33cf3a438a21f4b  /home/nestgate/Development/syntheticChemistry/agentReagents/images/cloud/ubuntu-22.04-server-cloudimg-amd64.img
bfa6ba63b2745ace87b2cdd4900de59dca339272a8fdf60a80f1702036b71178  /home/nestgate/Development/syntheticChemistry/agentReagents/debs/remote-desktop/rustdesk-1.2.3-x86_64.deb
bc854fdda8975ab18d43c411344a2676df1f8da1fd70750a3e53649c756bf059  /home/nestgate/Development/syntheticChemistry/agentReagents/isos/pop-os_22.04_amd64_nvidia_22.iso
