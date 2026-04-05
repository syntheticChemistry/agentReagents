# agentReagents — ecoPrimals Onboarding Handoff

**Date**: March 28, 2026
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

## Known Debt

1. **Path references** — Historical docs used machine-specific paths; scrubbed to `/path/to/...` placeholders (see archive docs)
2. **Missing .gitkeep files** — `.gitignore` re-includes `images/**/.gitkeep` but the files don't exist, so empty dirs aren't preserved in clones
3. **Missing install-deps.sh** — README references `scripts/install-deps.sh` which doesn't exist
4. **CHANGELOG location** — Only under `docs/CHANGELOG.md`, not at root per ecoPrimals convention
5. **License file** — No LICENSE file, should match benchScale (MIT OR Apache-2.0)
6. **docs/CHECKSUMS.md** — Was called out for old paths; file now uses repo-relative paths only

## Next Steps

1. Fix path references in README and docs (nestgate → eastgate, syntheticChemistry → ecoPrimals/infra)
2. Create .gitkeep files for empty artifact directories
3. Add root CHANGELOG.md (or symlink to docs/CHANGELOG.md)
4. Add LICENSE file
5. Remove or create the missing install-deps.sh script
6. Update docs/CHECKSUMS.md paths
