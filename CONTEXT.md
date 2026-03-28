# SPDX-License-Identifier: MIT OR Apache-2.0

# agentReagents — Context

## What

agentReagents is a shared artifact repository for binaries, ISOs, VM images,
cloud-init configs, and template-building scripts used across the ecoPrimals
ecosystem. Large artifacts are gitignored; scripts and documentation are
tracked. Automated setup downloads everything needed for a new gate.

## Role

Infrastructure supply chain for the ecoPrimals ecosystem. agentReagents
provides the base materials that benchScale consumes to create lab
environments (cloud images, cloud-init configs) and that gate deployments
consume for initial provisioning (ISOs, desktop templates, RustDesk configs).

Not a Rust crate — this is a script-and-config repository organized
LMS-style for both AI agent and human access.

## Architecture

- **scripts/** — Automated setup, download, and template-building scripts (9 active + 9 legacy in `scripts/legacy/`)
- **configs/** — Cloud-init YAML configs for VM provisioning (ecoPrimals gate nodes); includes `defaults.env` for shared paths and tunables
- **docs/** — CHANGELOG, MANIFEST, CHECKSUMS, SOURCES
- **bins/** — Compiled binaries and executables (gitignored, populated by scripts)
- **debs/** — Debian packages (gitignored, populated by scripts)
- **isos/** — ISO images for VM creation (gitignored, populated by scripts)
- **images/** — VM disk images: base, templates, intermediates, cloud (gitignored)

## Key Scripts

Active scripts live under `scripts/`. Legacy Pop!_OS– and RustDesk–oriented builders and helpers are kept in `scripts/legacy/` for reference; prefer the golden-path flows in active scripts for new work.

| Script | Purpose |
|--------|---------|
| `setup-reagents.sh` | One-command setup: creates dirs, downloads all artifacts |
| `verify-setup.sh` | Validates artifact integrity (including SHA256 vs `docs/CHECKSUMS.md`) and completeness |
| `download-cloud-images.sh` | Fetches Ubuntu cloud images |
| `download-isos.sh` | Fetches full ISO images for VM installation |
| `download-packages.sh` | Fetches RustDesk and other debs |
| `download-common.sh` | Shared download utilities (sourced by other scripts) |
| `build-cosmic-cloud-automated.sh` | Automated COSMIC desktop VM template build from cloud image |
| `lint.sh` | Syntax-checks all active and legacy shell scripts |
| `update-checksums.sh` | Regenerates `docs/CHECKSUMS.md` for downloaded artifacts |

## Boundaries

- agentReagents does NOT build primal binaries — those come from primal repos and land in plasmidBin.
- agentReagents does NOT orchestrate labs — that is benchScale's job.
- agentReagents does NOT contain Rust code — it is bash scripts, YAML configs, and documentation.
- Large binary artifacts (ISOs, qcow2 images, debs) are NEVER committed to git.

## Integration

- **benchScale** consumes cloud-init configs from `configs/` and cloud images from `images/` for libvirt VM provisioning
- **Gate deployments** use ISOs and template scripts for initial tower setup
- **plasmidBin** is a sibling under `infra/` — agentReagents provides the OS substrate, plasmidBin provides the primal binaries

## Status

Active — hardened bash (`set -euo pipefail`), `configs/defaults.env` for shared defaults, SHA256 verification in `verify-setup.sh`, and `scripts/lint.sh` for CI-style checks. Cloud-init config for ecoPrimals gate nodes is in tree. Golden-path template build remains COSMIC-from-cloud; legacy ISO/Pop!_OS flows live under `scripts/legacy/`.

## Ecosystem Position

```
infra/agentReagents  — THIS: base images, ISOs, cloud-init, templates
infra/benchScale     — lab substrate that consumes agentReagents images
infra/plasmidBin     — primal binaries deployed into benchScale labs
springs/primalSpring — validation experiments running in those labs
```

agentReagents is the supply chain.
benchScale is the lab builder.
plasmidBin is the genome carrier.
primalSpring is the validation authority.
