# agentReagents — wateringHole

**Status**: Active — March 28, 2026
**Org**: ecoPrimals (infra and ecosystem)

---

## Role

agentReagents is the supply chain for ecoPrimals gate provisioning. Scripts
and configs in git, large artifacts downloaded on demand. Provides base
images for benchScale labs and ISOs/templates for physical gate deployment.
Older Pop!_OS– and ISO-based template flows live under `scripts/legacy/` for
reference; active automation stays in `scripts/`.

## Current Capabilities

- Automated setup and verification (`setup-reagents.sh`, `verify-setup.sh`)
- **SHA256 verification** of downloaded artifacts against `docs/CHECKSUMS.md` (in `verify-setup.sh`)
- **`scripts/lint.sh`** — syntax checks all active and legacy shell scripts (optional shellcheck when installed)
- **`scripts/update-checksums.sh`** — regenerates checksum entries for present artifacts
- **`configs/defaults.env`** — shared defaults (paths, `VM_USER` / `VM_PASSWORD`, cloud image names, port range) sourced by active scripts
- Cloud image downloads (Ubuntu 24.04/22.04 cloud images; ISO flows still via active download scripts)
- ISO downloads for VM installation
- COSMIC desktop VM template builder from cloud image (`build-cosmic-cloud-automated.sh`)
- RustDesk deb provisioning; legacy Ubuntu 22 + RustDesk VM template under `scripts/legacy/`
- Cloud-init config for ecoPrimals gate nodes (`ecoprimals-node.yaml`)
- Systemd service templates for primal lifecycle management
- Port forwarding setup for NAT-ed nodes

## Ecosystem Integration

agentReagents feeds into benchScale's libvirt backend (Tier 2 validation):
```
agentReagents configs → benchScale QEMU labs → primal binaries deployed → experiments run
```

For Docker-tier validation (Tier 1), benchScale operates independently.
For physical gate deployment, agentReagents provides ISOs and cloud-init
configs directly to the provisioning flow.

## Handoffs

### Active

| File | Date | Scope |
|------|------|-------|
| [AGENTREAGENTS_ECOPRIMALS_ONBOARDING_HANDOFF_MAR28_2026.md](handoffs/AGENTREAGENTS_ECOPRIMALS_ONBOARDING_HANDOFF_MAR28_2026.md) | 2026-03-28 | ecoPrimals integration, path cleanup, standard alignment |

### Archived

_None yet._

## Convention

**Naming**: `AGENTREAGENTS_{TOPIC}_HANDOFF_{DATE}.md`

**Flow**: agentReagents → benchScale (cloud images + cloud-init for libvirt VMs).
agentReagents → gate deployments (ISOs + templates for physical machines).
