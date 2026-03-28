# agentReagents Evolution Specification

**Date**: March 28, 2026
**Version**: 2.0.0 (target)
**Status**: Specification — phased execution roadmap
**Current**: 1.2.0 — Phase A complete; Phase B partially done (legacy scripts relocated; `--minimal` not yet implemented)

---

## Purpose

Define the tractable evolution path from agentReagents 1.1.0 (functional artifact repository with stale branding and unverified integrity) to 2.0.0 (reliable, auditable infrastructure supply chain for ecoPrimals gate provisioning). Each phase is independently valuable.

## Architecture: Current vs Target

### Current (1.2.0) — post–Phase A

**Phase A (cleanup) — DONE**

- **Legacy scripts** moved to `scripts/legacy/` (9 scripts): Pop!_OS/ISO/RustDesk-era builders and configure/finalize pairs; **9 active scripts** remain in `scripts/`.
- **`verify-setup.sh`** validates SHA256 checksums against machine-readable `docs/CHECKSUMS.md` (not only size/existence).
- **ionChannel references** removed from active scripts; ecosystem pointers use benchScale / primalSpring.
- **Cloud-init** (`configs/ecoprimals-node.yaml`): duplicate `iproute2` removed; `ports.env` generation and related fixes applied per spec.
- **Stale docs** moved under `archive/docs/`.
- **Shell hygiene**: `set -euo pipefail` on all active scripts.
- **`configs/defaults.env`** created and sourced by active scripts for shared paths and tunables (e.g. `VM_*`, cloud image names).

**Phase B (golden path / simplification) — partially done**

- **Done**: Legacy pipelines isolated under `scripts/legacy/`; active tree matches the “keep” list from the Phase B spec.
- **Not done yet**: `setup-reagents.sh --minimal` flag and the simplified flow that skips full ISO/package downloads when only the golden path is needed.

### Baseline (1.1.0) — historical

Previously: 16 shell scripts with overlapping template pipelines from the ionChannel era; `verify-setup.sh` did not verify checksums; integrity lived only in prose/docs.

### Target (2.0.0)

A streamlined supply chain with one golden-path template build, verified artifacts, and ecoPrimals branding throughout:

- **One golden path**: Ubuntu 24.04 cloud image → cloud-init automated build → ecoPrimals gate VM
- **Verified artifacts**: `verify-setup.sh` checks SHA256 from machine-readable `docs/CHECKSUMS.md`
- **Clean branding**: All references to ionChannel/syntheticChemistry replaced with ecoPrimals/benchScale/primalSpring
- **Archived legacy**: Pop!_OS 22.04 flows, desktop template flows, and dated session notes moved to `archive/`

---

## Phase A: Cleanup (no behavior changes)

### A1. Populate `docs/MANIFEST.md`

Currently a mix of template headings ("*(No entries yet)*") and a few concrete entries at the bottom. Populate from what the download scripts actually fetch:

| Category | Artifact | Source Script |
|----------|----------|--------------|
| Cloud Images | `images/cloud/ubuntu-22.04-server-cloudimg-amd64.img` | `download-cloud-images.sh` |
| Cloud Images | `images/cloud/ubuntu-24.04-server-cloudimg-amd64.img` | `download-cloud-images.sh` |
| Debs | `debs/remote-desktop/rustdesk-1.2.3-x86_64.deb` | `download-packages.sh` |
| ISOs | `isos/pop-os_22.04_amd64_nvidia_22.iso` | `download-isos.sh` |
| ISOs | `isos/ubuntu-24.04.1-desktop-amd64.iso` | `download-isos.sh` |
| Configs | `configs/ecoprimals-node.yaml` | Tracked in git |

### A2. Fix `verify-setup.sh` to check checksums

Add SHA256 verification after the existing existence checks:

```bash
CHECKSUMS_FILE="$REAGENTS_DIR/docs/CHECKSUMS.md"
if [ -f "$CHECKSUMS_FILE" ]; then
    log "Verifying checksums..."
    grep -E '^[0-9a-f]{64}  ' "$CHECKSUMS_FILE" | while read -r line; do
        expected_hash=$(echo "$line" | awk '{print $1}')
        filepath=$(echo "$line" | awk '{print $2}')
        if [ -f "$REAGENTS_DIR/$filepath" ]; then
            actual_hash=$(sha256sum "$REAGENTS_DIR/$filepath" | awk '{print $1}')
            if [ "$expected_hash" = "$actual_hash" ]; then
                log "  PASS: $filepath"
            else
                log_warn "  FAIL: $filepath (checksum mismatch)"
            fi
        fi
    done
fi
```

Make `docs/CHECKSUMS.md` dual-format: Markdown headings for humans, but checksum lines in `sha256sum`-compatible format (`<hash>  <relative-path>`). Strip prose from the parseable section.

### A3. Replace ionChannel references

Seven scripts contain ionChannel references in "next steps" echo output or comments:

| Script | Reference Type |
|--------|---------------|
| `setup-reagents.sh` | Next step: `cd ../ionChannel && cargo run --bin ab-validation` |
| `verify-setup.sh` | Same |
| `finalize-popos-template.sh` | `cd ../../ionChannel` |
| `finalize-popos-24-template.sh` | ionChannel validation banner + cd |
| `build-popos-24-template.sh` | Banner: "Primary Target for ionChannel Validation" |
| `build-popos-cosmic-template.sh` | Comment + cd ionChannel |
| `build-cosmic-cloud-automated.sh` | `cd ../../ionChannel` |

Replace all with ecoPrimals-ecosystem equivalents:
- `cd ../ionChannel && cargo run --bin ab-validation` → `cd ../benchScale && ./scripts/create-lab.sh <topology>`
- "ionChannel Validation" → "ecoPrimals Validation"
- Path fixes: `../../ionChannel` → `../benchScale` (both are siblings under `infra/`)

### A4. Fix cloud-init YAML issues

In `configs/ecoprimals-node.yaml`:

1. **Duplicate `iproute2` package**: Remove the second occurrence (lines ~33 and ~38)
2. **Placeholder SSH key**: Add a clear comment block explaining how to replace, and add a check in `verify-setup.sh` that warns if `CHANGEME` is still present
3. **Systemd `ExecStart` env expansion**: Document that `${PORT}` and `${FAMILY_ID}` require the corresponding `EnvironmentFile` to exist. Add a setup step in `runcmd` that generates a default `ports.env` if missing
4. **`setup-ports.sh` syntax error**: The iptables fallback block has a `)` instead of `done` — fix the for loop termination

### A5. Archive stale docs

Move to `archive/docs/`:

| File | Reason |
|------|--------|
| `TEMPLATE_BUILD_STATUS.md` | Session note, not maintained |
| `GIT_READY_SUMMARY_DEC_28_2025.md` | Dated push checklist, historical only |
| `GIT_SETUP.md` | Overlaps with README, references stale paths |
| `CLOUD_INIT_READY.md` | One-off "ready" doc, superseded by CONTEXT.md |

Keep but update:
- `SETUP.md` — still useful as the setup guide, update paths
- `ISO_DOWNLOAD_LINKS.md` — reference material, update branding
- `MULTI_DISTRO_STRATEGY.md` — architecture reference, update branding
- `POPOS_24_COSMIC_GUIDE.md` / `POPOS_24_INSTALL_GUIDE.md` / `POPOS_TEMPLATE_GUIDE.md` — mark as legacy in headers

---

## Phase B: Golden Path Selection

### B1. Define the golden path

**Winner: Ubuntu 24.04 cloud image + cloud-init automated build**

This is the only path that:
- Works with both benchScale Docker (Tier 1) and libvirt/qemu (Tier 2)
- Produces headless VMs suitable for primal deployment
- Requires no manual interaction (ISO-based flows need human input)
- Aligns with `ecoprimals-node.yaml` cloud-init config

**Supported pipeline**:
```
download-cloud-images.sh → images/cloud/ubuntu-24.04-server-cloudimg-amd64.img
                         → build-cosmic-cloud-automated.sh (if COSMIC desktop needed)
                         → OR direct cloud-init with ecoprimals-node.yaml (for gates)
```

### B2. Mark legacy pipelines

Scripts to mark with `# LEGACY` header and move to `scripts/legacy/`:
- `build-popos-from-iso.sh` — Pop!_OS 22.04 ISO-based (requires manual install)
- `build-popos-24-template.sh` — Pop!_OS 24.04 ISO-based
- `build-popos-cosmic-template.sh` — mixed cloud/ISO approach
- `build-rustdesk-template.sh` — Ubuntu 22.04 only
- `configure-popos-template.sh` / `finalize-popos-template.sh` — Pop!_OS 22.04 pair
- `configure-popos-24-template.sh` / `finalize-popos-24-template.sh` — Pop!_OS 24.04 pair
- `download-popos-24-cosmic.sh` — Pop!_OS COSMIC alpha scaffold

Keep in `scripts/` (active):
- `setup-reagents.sh` — entry point
- `verify-setup.sh` — integrity checking
- `download-common.sh` — shared helpers
- `download-cloud-images.sh` — golden path download
- `download-isos.sh` — still needed for desktop ISOs
- `download-packages.sh` — RustDesk deb
- `build-cosmic-cloud-automated.sh` — golden path build (if COSMIC desktop needed)

### B3. Simplify `setup-reagents.sh`

Current flow downloads everything (ISOs, cloud images, debs). Add a `--minimal` flag that only downloads the golden path:

```bash
if [ "$MINIMAL" = true ]; then
    source "$SCRIPTS_DIR/download-cloud-images.sh"
    log "Minimal setup complete — Ubuntu 24.04 cloud image ready for benchScale"
else
    source "$SCRIPTS_DIR/download-cloud-images.sh"
    source "$SCRIPTS_DIR/download-isos.sh"
    source "$SCRIPTS_DIR/download-packages.sh"
fi
```

---

## Phase C: Quality Gates

### C1. Syntax validation

Add a `scripts/lint.sh` that runs:
```bash
for script in scripts/*.sh; do
    bash -n "$script" || exit 1
done
```

Document the shellcheck target for environments that have it:
```bash
if command -v shellcheck &>/dev/null; then
    shellcheck scripts/*.sh
fi
```

### C2. Config validation

`verify-setup.sh` should also validate:
- `configs/ecoprimals-node.yaml` is valid YAML (if `python3 -c 'import yaml'` is available)
- No `CHANGEME` placeholders remain in configs intended for production use
- All scripts referenced in MANIFEST.md exist and are executable

### C3. Checksum generation

Add a `scripts/update-checksums.sh` that regenerates `docs/CHECKSUMS.md`:
```bash
echo "# Artifact Checksums" > "$CHECKSUMS_FILE"
echo "" >> "$CHECKSUMS_FILE"
echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$CHECKSUMS_FILE"
echo "" >> "$CHECKSUMS_FILE"
for f in images/cloud/*.img debs/**/*.deb isos/*.iso; do
    [ -f "$f" ] && sha256sum "$f" >> "$CHECKSUMS_FILE"
done
```

---

## Phase D: Config Centralization

### D1. Shared defaults

Create `configs/defaults.env` sourced by all scripts:

```bash
# agentReagents shared defaults
REAGENTS_USER="ecoprimals"
REAGENTS_DIR="${REAGENTS_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
LIBVIRT_IMAGES="/var/lib/libvirt/images"
PRIMAL_PORT_RANGE_START=9100
PRIMAL_PORT_RANGE_END=9800
CLOUD_IMAGE_UBUNTU_2404="ubuntu-24.04-server-cloudimg-amd64.img"
CLOUD_IMAGE_UBUNTU_2204="ubuntu-22.04-server-cloudimg-amd64.img"
```

### D2. Script header standardization

All active scripts should source defaults and use consistent patterns:
```bash
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/../configs/defaults.env" 2>/dev/null || true
```

---

## Script Inventory with Disposition

| Script | Lines | Golden Path | Disposition |
|--------|-------|-------------|-------------|
| `setup-reagents.sh` | ~200 | Yes | Keep, add `--minimal` |
| `verify-setup.sh` | ~150 | Yes | Keep, add checksums |
| `download-common.sh` | ~50 | Yes | Keep |
| `download-cloud-images.sh` | ~80 | Yes | Keep |
| `download-isos.sh` | ~60 | Support | Keep |
| `download-packages.sh` | ~40 | Support | Keep |
| `build-cosmic-cloud-automated.sh` | ~200 | Yes | Keep |
| `build-popos-from-iso.sh` | ~150 | No | → `scripts/legacy/` |
| `build-popos-24-template.sh` | ~180 | No | → `scripts/legacy/` |
| `build-popos-cosmic-template.sh` | ~160 | No | → `scripts/legacy/` |
| `build-rustdesk-template.sh` | ~140 | No | → `scripts/legacy/` |
| `configure-popos-template.sh` | ~100 | No | → `scripts/legacy/` |
| `configure-popos-24-template.sh` | ~100 | No | → `scripts/legacy/` |
| `finalize-popos-template.sh` | ~80 | No | → `scripts/legacy/` |
| `finalize-popos-24-template.sh` | ~80 | No | → `scripts/legacy/` |
| `download-popos-24-cosmic.sh` | ~40 | No | → `scripts/legacy/` |

## Stale Documentation Inventory

| File | Status | Action |
|------|--------|--------|
| `TEMPLATE_BUILD_STATUS.md` | Session note | → `archive/docs/` |
| `GIT_READY_SUMMARY_DEC_28_2025.md` | Dated checklist | → `archive/docs/` |
| `GIT_SETUP.md` | Overlaps README | → `archive/docs/` |
| `CLOUD_INIT_READY.md` | Superseded | → `archive/docs/` |
| `SETUP.md` | Useful | Update paths |
| `ISO_DOWNLOAD_LINKS.md` | Reference | Update branding |
| `MULTI_DISTRO_STRATEGY.md` | Architecture | Update branding |
| `POPOS_24_COSMIC_GUIDE.md` | Legacy build | Add legacy header |
| `POPOS_24_INSTALL_GUIDE.md` | Legacy build | Add legacy header |
| `POPOS_TEMPLATE_GUIDE.md` | Legacy build | Add legacy header |

## Success Criteria

- **Phase A**: ~~All ionChannel references replaced, MANIFEST.md populated, verify-setup.sh checks SHA256, cloud-init YAML clean, stale docs archived~~ **Met (1.2.0)**
- **Phase B**: Golden path documented and marked, legacy scripts in `scripts/legacy/` (**done**); `setup-reagents.sh --minimal` (**not yet**)
- **Phase C**: `scripts/lint.sh` passes on all active scripts (**done**), checksum generation automated via `update-checksums.sh` (**done**)
- **Phase D**: All active scripts source `configs/defaults.env`, consistent headers (**done** for active scripts)
