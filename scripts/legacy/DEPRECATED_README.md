# Deprecated Bash Scripts

These scripts have been deprecated in favor of the modern Rust-based image builder.

## Why Deprecated?

### Problems with Bash Scripts:
- ❌ No verification of cloud-init completion
- ❌ Only checked if VM powered off (not if build succeeded)
- ❌ Blocking sleep loops (not async)
- ❌ No state machine
- ❌ No progress reporting
- ❌ Created templates even when builds failed
- ❌ Error-prone string manipulation

### Replaced By:
**Modern Rust Builder** (`agentReagents/src/builder/`)
- ✅ Async/await (non-blocking)
- ✅ State machine with type safety
- ✅ Mandatory verification before template creation
- ✅ Proper error handling
- ✅ Progress tracking
- ✅ Observability

## Migration

### Old Way (Bash):
```bash
./scripts/build-popos-24-cosmic-rustdesk.sh
# Waits 30+ minutes
# Creates template (might be broken, no verification)
```

### New Way (Rust):
```bash
cd agentReagents
cargo run --example build_cosmic_desktop
# Shows progress with state machine
# Verifies installation before template creation
# Fails early with clear error messages
```

## Scripts Deprecated

- `build-popos-24-cosmic-rustdesk.sh` - Main COSMIC builder
- `build-popos-24-cosmic-baseline.sh` - Baseline builder
- `build-cosmic-cloud-automated.sh` - Automated builder
- `build-popos-cosmic-monitored.sh` - Monitored builder
- `build-popos-cosmic-template.sh` - Template builder
- `build-popos-from-iso.sh` - ISO-based builder
- `build-rustdesk-template.sh` - RustDesk builder
- `install-cosmic-rustdesk.sh` - Installation script
- `create-cosmic-rustdesk-vm.sh` - VM creation
- `configure-popos-24-template.sh` - Configuration
- `finalize-popos-24-template.sh` - Finalization

## Kept Scripts

These utility scripts are still valid:
- `download-cloud-images.sh` - Download base images
- `download-isos.sh` - Download ISOs
- `download-packages.sh` - Download packages
- `setup-reagents.sh` - Initial setup
- `verify-setup.sh` - Verify installation

## Timeline

- **Dec 28, 2025**: Bash scripts created
- **Dec 30, 2025**: Deep debt identified, Rust solution created
- **Dec 30, 2025**: Scripts deprecated, moved to deprecated/

## See Also

- `../../DEEP_DEBT_ANALYSIS.md` - Full analysis of bash script issues
- `../../DEEP_DEBT_SOLVED.md` - Rust solution architecture
- `../../src/builder/` - Modern Rust implementation

