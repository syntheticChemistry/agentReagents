# agentReagents — ecoPrimals Onboarding Handoff

**Date**: March 28, 2026 (handoff); **Updated**: April 5, 2026 (post–deep-debt sprint)
**Author**: Agent-assisted
**Scope**: Bringing agentReagents up to ecoPrimals standards and integrating with benchScale validation substrate

---

## What Was Done

### ecoPrimals Standard Alignment

- **CONTEXT.md** — Added with What/Role/Architecture/Boundaries/Status/Ecosystem format
- **specs/ARCHITECTURE.md** — Artifact management architecture, script inventory, consumption patterns, integration flow
- **wateringHole/** — This handoff structure with README and naming convention

### Cloud-Init Config (`configs/ecoprimals-node.yaml`)

Cloud-init config for ecoPrimals gate VMs (benchScale Tier 2 libvirt path):
- Creates `ecoprimals` user with SSH key and sudo
- Sets up `/opt/ecoprimals/{bin,graphs,config,logs}` directory structure
- Installs minimal networking diagnostics (net-tools, iproute2, curl, jq)
- Includes systemd service template `ecoprimals@.service` for primal lifecycle
- Opens ports 9100-9800 for primal TCP endpoints
- Firewall setup script for UFW/iptables

### Deep debt sprint (2026-04, v0.2.0)

See root `CHANGELOG.md`. Highlights: **60.2%** coverage (89 tests), capability-based **RegistrationSettings**, `#[expect]` policy, root changelog + `deny.toml`, `tarpaulin` gate, archive path scrub, README/security/build docs.

## Known Debt

1. **Empty artifact dirs** — `.gitignore` re-includes `images/**/.gitkeep` but some `.gitkeep` files may still be missing, so empty dirs are not always preserved in fresh clones.
2. **Optional tooling** — Confirm whether a dedicated `scripts/install-deps.sh` is still desired; README no longer depends on it for core Rust workflow (`cargo build` / `cargo test`).

Resolved since March 2026: root `CHANGELOG.md` is canonical (`docs/HISTORY.md` holds Dec 2025 bootstrap notes); LICENSE present; machine-specific paths scrubbed in archive; changelog/CHECKSUMS conventions aligned.

## Next Steps

1. Add or verify `.gitkeep` files under gitignored artifact trees if preserving empty directories in clones matters for your workflow.
2. Keep `CONTEXT.md` and root `CHANGELOG.md` in sync when cutting releases or changing coverage gates.
3. Continue benchScale / gate integration testing against published templates.
