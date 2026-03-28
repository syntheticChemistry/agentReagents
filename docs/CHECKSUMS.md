# agentReagents Checksums

SHA256 checksums for verifying artifact integrity.

## How to Use

Automated verification:
```bash
bash scripts/verify-setup.sh
```

Manual verification of a single file:
```bash
sha256sum <file> | diff - <(grep <file> docs/CHECKSUMS.md)
```

## How to Add

```bash
sha256sum path/to/file >> docs/CHECKSUMS.md
```

---

## Checksums

2cbbe814d84c9dc7d749ea0afda924d9e26f771a3f824b48f33cf3a438a21f4b  images/cloud/ubuntu-22.04-server-cloudimg-amd64.img
bfa6ba63b2745ace87b2cdd4900de59dca339272a8fdf60a80f1702036b71178  debs/remote-desktop/rustdesk-1.2.3-x86_64.deb
bc854fdda8975ab18d43c411344a2676df1f8da1fd70750a3e53649c756bf059  isos/pop-os_22.04_amd64_nvidia_22.iso
