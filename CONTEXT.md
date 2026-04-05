# SPDX-License-Identifier: AGPL-3.0-only

# agentReagents — Context

## What

agentReagents is a Rust crate and infrastructure supply chain for the ecoPrimals
ecosystem. It builds reproducible VM substrates from YAML template manifests,
manages artifact repositories (ISOs, cloud images, driver reagents), and provides
a JSON-RPC 2.0 server mode for programmatic access. Large binary artifacts are
gitignored; scripts, templates, and documentation are tracked.

## Role

Infrastructure supply chain for the ecoPrimals ecosystem. agentReagents
provides the base materials that benchScale consumes to create lab
environments (cloud images, cloud-init configs) and that gate deployments
consume for initial provisioning (ISOs, templates, driver reagents).

## Status (2026-04)

- **Release**: v0.2.0 (see root `CHANGELOG.md`)
- **Quality**: Grade A maintenance posture; **60.2%** line coverage (89 lib tests), `tarpaulin` fail-under 60%
- **Registration**: Capability-based `RegistrationSettings` (no hardcoded Songbird-only paths)
- **Lint policy**: `#[expect(...)]` with reasons (no bare `#[allow]`)

## Architecture

- **src/** — Rust crate: template builder, verification engine, JSON-RPC server
- **templates/** — YAML manifests for VM builds, including gate templates
- **scripts/** — Automated setup, download, and template-building scripts (see `scripts/README.md`)
- **configs/** — Cloud-init YAML configs for VM provisioning
- **docs/** — `HISTORY.md` (bootstrap-era notes), MANIFEST, CHECKSUMS, SOURCES; canonical changelog is root `CHANGELOG.md`
- **bins/** — Compiled binaries (gitignored, populated by scripts)
- **packages/** — Downloaded packages (gitignored, downloaded by scripts)
- **isos/** — ISO images (gitignored, populated by scripts)
- **images/** — VM disk images (gitignored, built by template pipeline)

## Boundaries

- agentReagents does NOT build primal binaries — those come from plasmidBin.
- agentReagents does NOT orchestrate labs — that is benchScale's job.
- Large binary artifacts (ISOs, qcow2 images, debs) are NEVER committed to git.

## Integration

- **benchScale** consumes cloud-init configs and cloud images for VM provisioning
- **Gate deployments** use templates for initial tower setup
- **plasmidBin** is a sibling under `infra/` — agentReagents provides the OS substrate, plasmidBin provides the primal binaries

## Ecosystem Position

```
infra/agentReagents  — THIS: templates, VM builder, artifact supply chain
infra/benchScale     — lab substrate that consumes agentReagents images
infra/plasmidBin     — primal binaries deployed into benchScale labs
springs/primalSpring — validation experiments running in those labs
```

agentReagents is the supply chain.
benchScale is the lab builder.
plasmidBin is the genome carrier.
primalSpring is the validation authority.
