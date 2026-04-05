# agentReagents

**Template-driven VM image builder for the ecoPrimals ecosystem**

🟢 **Status**: Production Ready — v0.1.0, Rust 2024 edition  
📅 **Last Updated**: March 29, 2026  
🧪 **Tests**: 33 lib + 6 integration passing  
🔒 **Safety**: `forbid(unsafe_code)`, `deny(clippy::unwrap_used)`, `clippy::pedantic` + `clippy::nursery`  
📜 **License**: AGPL-3.0-or-later (scyBorg Provenance Trio)

## Security Note

Template files in `templates/` contain example passwords (e.g. `iontest123`, `poptest123`, `ubuntu123`)
that are **lab-only defaults for development VMs**. Never use these in production environments.
Always replace template credentials before deploying to any network-accessible system.

---

## What is agentReagents?

agentReagents builds reproducible, validated VM substrates from YAML template manifests. It orchestrates cloud-init provisioning, post-boot synthesis via SSH, multi-method verification, and registry management. Designed for gate deployments in the ecoPrimals ecosystem.

**Ecosystem role**: Provisions VM images for hardware gates (Eastgate, biomeGate, pixelGate) and provides driver reagent isolation for GPU sovereign compute (hotSpring use case).

## Features

- **YAML template manifests** for declarative, reproducible builds
- **Cloud-init + post-boot synthesis** hybrid approach
- **JSON-RPC 2.0 server** (`agent-reagents server --port PORT`) per UniBin standard
- **Gate provisioning templates** for biomeOS, GPU sovereign, and aarch64 pixelGate
- **Multi-method verification** (dpkg-query, dpkg -l, apt-cache, dependency checks)
- **Template registry** with checksums and verification
- **plasmidBin integration** for baking primal binaries into VM images
- **VM senescence monitoring** during builds via benchScale

## Build Requirements

agentReagents depends on benchScale via path dependency (`../benchScale`). Clone both repositories as siblings:

```bash
mkdir ecoPrimals && cd ecoPrimals
git clone https://github.com/syntheticChemistry/benchScale.git
git clone https://github.com/syntheticChemistry/agentReagents.git
cd agentReagents
cargo build
```

## Quick Start

```bash
cargo build --release
cargo test

# Validate a template manifest
cargo run --bin agent-reagents -- validate templates/gates/gate-ubuntu24-biomeos.yaml

# Start JSON-RPC server
cargo run --bin agent-reagents -- server --port 9201

# Clean up orphaned VMs
cargo run --bin lab-cleanup
```

## JSON-RPC Methods

When running in server mode (`agent-reagents server --port PORT`):

| Method | Description |
|--------|-------------|
| `health.liveness` | Mandatory liveness probe |
| `health.readiness` | Readiness with registry status |
| `health.check` | Full health with template count |
| `registry.list` | List registered templates |
| `template.validate` | Validate a manifest file |
| `image.list` | List built VM images |

## Gate Templates

Pre-built templates for ecoPrimals gate deployments:

| Template | Purpose | Arch |
|----------|---------|------|
| `gate-ubuntu24-biomeos` | Standard biomeOS gate (Songbird + BearDog + NestGate) | x86_64 |
| `gate-ubuntu24-gpu-sovereign` | GPU VFIO passthrough with glowplug | x86_64 |
| `gate-aarch64-pixelgate` | ARM64 mobile gate for Pixel devices | aarch64 |

## Code Structure

```
src/
├── bin/
│   ├── agent-reagents.rs    # Main CLI
│   └── lab-cleanup.rs       # VM cleanup utility
├── builder/
│   ├── mod.rs               # Build orchestration
│   ├── cloud_init_monitor.rs
│   ├── post_boot.rs         # SSH-based synthesis
│   ├── verification.rs      # Multi-method verification
│   ├── vm_handle.rs         # VM interface
│   └── vm_reboot.rs         # Reboot handling
├── server/
│   └── mod.rs               # JSON-RPC 2.0 server
├── templates/
│   ├── manifest.rs          # Template structure
│   └── registry.rs          # Template registry
├── images.rs                # Image management
├── packages.rs              # Package handling
└── lib.rs                   # Public API

templates/
├── gates/                   # ecoPrimals gate templates
└── *.yaml                   # Additional templates
```

## Configuration

Uses benchScale's configuration system:

```bash
BENCHSCALE_LIBVIRT_URI="qemu:///system"
BENCHSCALE_VM_IMAGES_DIR="/var/lib/libvirt/images"
BENCHSCALE_MONITORING_MAX_FAILURES=180
```

## Related Projects

- **[benchScale](../benchScale/)** — VM orchestration backend (dependency)
- **[wateringHole](../wateringHole/)** — ecoPrimals inter-project standards
- **[plasmidBin](../plasmidBin/)** — Binary distribution for primals

## License

AGPL-3.0-or-later — Part of the ecoPrimals ecosystem.

---

Made with Rust by the ecoPrimals ecosystem
