# agentReagents - Template-Driven VM Image Builder

**Modern Rust infrastructure for building, testing, and validating VM substrates**

🟢 **Status**: Production Ready - Evolution #23 Complete  
📅 **Last Updated**: January 8, 2026  
🎉 **Validation**: ✅ 2/5 substrates validated (57/57 checks = 100%)

---

## Quick Start

```bash
# Build a substrate from template
cd agentReagents
cargo run --bin agent-reagents build templates/control-ubuntu24-rust-piecewise.yaml

# Clean up orphaned VMs
cargo run --bin lab-cleanup
```

---

## What is agentReagents?

**agentReagents** is a template-driven VM image builder that creates reproducible, validated VM substrates for testing and deployment. It orchestrates the entire VM lifecycle from base images to production-ready systems.

### Key Capabilities

- **Template-Driven Builds**: YAML-based manifests for reproducible substrates
- **Cloud-Init + Post-Boot Synthesis**: Hybrid approach for reliable installations
- **Piecewise Package Installation**: Granular control with validation at each step
- **Real-Time Progress Monitoring**: VM senescence tracking during builds
- **Robust Verification**: Multi-method package validation (Evolution #23)
- **Zero Unsafe Code**: Modern idiomatic Rust throughout

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│         Template Manifest (YAML)                        │
│  • Base image, packages, users, verification            │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│         agentReagents Builder                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Cloud-Init   │  │ Post-Boot    │  │ Verification │  │
│  │ Generation   │  │ Synthesis    │  │ Engine       │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│         benchScale Backend                              │
│  • VM provisioning • DHCP discovery • Health monitoring │
└─────────────────────────────────────────────────────────┘
```

---

## Features

### Template System

- **YAML Manifests**: Declarative substrate definitions
- **Resource Config**: Memory, vCPUs, disk, timeouts
- **User Management**: SSH keys, passwords, sudo access
- **Build Steps**: Cloud-init native package installation
- **Post-Boot Steps**: Complex installations via SSH
- **Verification**: Multi-method package/service validation

### Build Process

1. **Template Loading**: Parse and validate YAML manifest
2. **Cloud-Init Generation**: Create user-data with packages
3. **VM Provisioning**: benchScale creates VM with cloud-init
4. **Senescence Monitoring**: Real-time health tracking
5. **Post-Boot Synthesis**: Execute complex installation steps
6. **Verification**: Validate packages, services, files, commands
7. **Registration**: Record substrate in registry

### Monitoring & Diagnostics

- **Senescence Monitor**: Track VM health during builds
- **DHCP Lease Tracking**: Handle IP changes (Evolution #22)
- **Progress Reporting**: Real-time status updates
- **Failure Detection**: Stall detection and recovery
- **Boot Diagnostics**: Serial console and systemd logs (Evolution #13)

---

## Current Substrates

### ✅ Validated (Production Ready)

| Template | Components | Status | Checks |
|----------|-----------|--------|--------|
| **treatment-ubuntu24-ionchannel-rustdesk** | Ubuntu 24 + GNOME + ionChannel + RustDesk | ✅ Ready | 33/33 (100%) |
| **test-ubuntu24-rustdesk-only** | Ubuntu 24 + GNOME + RustDesk | ✅ Ready | 24/24 (100%) |

### 🔍 Under Investigation

| Template | Components | Status | Issue |
|----------|-----------|--------|-------|
| **test-ubuntu24-ionchannel-piecewise** | Ubuntu 24 + ionChannel (test) | 🔴 Reboot failure | No route to host |
| **test-ubuntu24-fulldesktop-rustdesk** | Ubuntu 24 + Full desktop + RustDesk | 🔴 Reboot failure | No route to host |
| **ctrl-popos-cosmic-piecewise** | Pop!_OS 24 + COSMIC | 🟡 PPA issue | Ubuntu 24.04 incompatibility |

### 📋 Control Substrate

| Template | Components | Purpose |
|----------|-----------|---------|
| **control-ubuntu24-rust-piecewise** | Ubuntu 24 + GNOME + RustDesk | A/B testing control group |

---

## Template Structure

```yaml
name: "treatment-ubuntu24-ionchannel-rustdesk"
version: "1.0.0"
base_image: "ubuntu-24.04-server-cloudimg-amd64.img"
description: "Treatment substrate for A/B testing"

resources:
  memory_mb: 8192
  vcpus: 4
  disk_gb: 50
  timeout_secs: 3600
  cloud_init_timeout_secs: 1800

users:
  - username: "testuser"
    ssh_authorized_keys:
      - "ssh-rsa AAAAB3..."
    sudo: "ALL=(ALL) NOPASSWD:ALL"

build_steps:
  - name: "install_packages"
    action: "cloud_init"
    packages:
      - gdm3
      - gnome-shell
      - gnome-session
      - xserver-xorg-core
      - libgtk-3-0t64  # Evolution #23: Ubuntu 24.04 package rename

post_boot_steps:
  - name: "Install ionChannel"
    command: "bash /tmp/install-ionchannel.sh"
    source_script: "scripts/install-ionchannel.sh"
    timeout_secs: 600

  - name: "Install RustDesk"
    command: "sudo dpkg -i /tmp/rustdesk-*.deb"
    source_file: "packages/rustdesk-1.3.3-x86_64.deb"
    timeout_secs: 300

  - name: "Reboot VM"
    command: "sudo systemctl reboot"
    expect_disconnect: true
    reboot: true

verification:
  required_packages:
    - gdm3
    - gnome-shell
    - rustdesk
    - libgtk-3-0t64
  required_services:
    - gdm
    - rustdesk
  required_files:
    - /usr/bin/rustdesk
  verification_commands:
    - "systemctl is-active gdm"
```

See [`specs/MANIFEST_SPEC.md`](specs/MANIFEST_SPEC.md) for complete specification.

---

## Recent Evolutions

### Evolution #23: Robust Package Verification ✅
**Status**: Complete & Validated  
**Impact**: Caught real installation failures

**Features**:
- Multi-method verification (dpkg-query, dpkg -l, apt-cache, dependencies)
- Architecture suffix handling (`:amd64`)
- Rich diagnostics for troubleshooting
- False negative detection and resolution

**Discovery**: "False negative" for `libgtk-3-0` caught real issue - Ubuntu 24.04 renamed package to `libgtk-3-0t64`. All templates updated.

### Evolution #22: DHCP Lease Renewal Tracking ✅
**Status**: Complete & Validated  
**Impact**: Prevents false negatives during long builds

**Features**:
- MAC address tracking in `SenescenceMonitor`
- Periodic IP re-discovery (every 100 seconds)
- Network verification uses current IP
- Handles DHCP lease changes gracefully

### Evolution #21: Configurable Failure Threshold ✅
**Status**: Complete & Validated  
**Impact**: 30-minute tolerance for cloud-init builds

**Features**:
- Configurable `max_failures` in `SenescenceMonitor`
- Workload presets: quick VMs (100s), desktop (10min), cloud-init (30min)
- Allows long-running package installations to complete

---

## Directory Structure

```
agentReagents/
├── src/
│   ├── bin/
│   │   ├── agent-reagents.rs    # Main builder CLI
│   │   └── lab-cleanup.rs       # VM cleanup utility
│   ├── builder/
│   │   ├── mod.rs               # Build orchestration
│   │   ├── cloud_init_monitor.rs # Cloud-init tracking
│   │   ├── post_boot.rs         # SSH-based synthesis
│   │   ├── verification.rs      # Multi-method verification
│   │   ├── vm_handle.rs         # VM interface
│   │   └── vm_reboot.rs         # Reboot handling
│   ├── templates/
│   │   ├── manifest.rs          # Template structure
│   │   └── registry.rs          # Substrate registry
│   ├── discovery.rs             # System discovery
│   ├── images.rs                # Image management
│   └── packages.rs              # Package handling
├── templates/
│   ├── control-ubuntu24-rust-piecewise.yaml
│   ├── treatment-ubuntu24-ionchannel-rustdesk.yaml
│   └── test-*.yaml              # Test templates
├── specs/
│   ├── ARCHITECTURE.md          # System architecture
│   ├── MANIFEST_SPEC.md         # Template specification
│   └── GUIDANCE.md              # Best practices
├── scripts/
│   ├── gather-ingredients.sh    # Download dependencies
│   ├── serve-ingredients.sh     # Local package server
│   └── deprecated/              # Old scripts (archived)
├── packages/                    # Local package cache
├── images/                      # VM images
│   ├── base/                    # Base OS images
│   ├── cloud/                   # Cloud-init images
│   └── templates/               # Built substrates
└── isos/                        # OS installation ISOs
```

---

## Configuration

agentReagents uses `benchScale` configuration system:

```bash
# Monitoring settings
export BENCHSCALE_MONITORING_CHECK_INTERVAL_SECS=10
export BENCHSCALE_MONITORING_STALL_THRESHOLD_SECS=60
export BENCHSCALE_MONITORING_MAX_FAILURES=180

# Timeout settings
export BENCHSCALE_CLOUD_INIT_TIMEOUT_SECS=1800
export BENCHSCALE_SSH_TIMEOUT_SECS=300

# Network settings
export BENCHSCALE_NETWORK_NAME="default"
export BENCHSCALE_DHCP_DISCOVERY_TIMEOUT_SECS=30

# Storage settings
export BENCHSCALE_VM_IMAGES_DIR="/var/lib/libvirt/images"
```

See `benchScale/src/config/` for complete configuration system.

---

## Development

### Build

```bash
# Development build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

### Create a New Template

1. Copy an existing template from `templates/`
2. Modify resources, packages, and verification
3. Test with `cargo run --bin agent-reagents build templates/your-template.yaml`
4. Validate all verification checks pass
5. Document in substrate registry

### Debugging

```bash
# Preserve VM on failure for inspection
export PRESERVE_VM_ON_FAILURE=1

# Monitor build with detailed logging
RUST_LOG=debug cargo run --bin agent-reagents build templates/test.yaml

# Extract diagnostics from preserved VM
# (VmGuard respects PRESERVE_VM_ON_FAILURE)
virsh console <vm-name>
```

---

## Integration with benchScale

agentReagents is a **consumer** of benchScale:

```rust
use benchscale::{LibvirtBackend, CloudInit, Backend};
use benchscale::config::Config as BenchScaleConfig;

// Create backend
let backend = Arc::new(LibvirtBackend::new()?);

// Generate cloud-init from template
let cloud_init = CloudInit::builder()
    .add_user(&manifest.users[0].username, &ssh_key)
    .packages(cloud_init_packages)
    .build();

// Provision VM
let node = backend.create_node(
    &vm_name,
    &base_image,
    manifest.resources.memory_mb,
    manifest.resources.vcpus,
    manifest.resources.disk_gb,
    Some(&cloud_init),
).await?;

// Monitor with senescence
let monitor = Arc::new(
    SenescenceMonitor::from_config(
        vm_name.clone(),
        node.ip_address.clone(),
        node.metadata.get("mac_address").cloned(),
        &config.monitoring.for_cloud_init_packages(),
    )
);
```

---

## Best Practices

### Template Design

1. **Use cloud-init for base packages**: Faster, more reliable
2. **Post-boot for complex installations**: ionChannel, RustDesk, custom setups
3. **Piecewise installation**: Small steps with validation
4. **Explicit package names**: Avoid meta-packages like `ubuntu-desktop-minimal`
5. **Ubuntu 24.04 packages**: Use `libgtk-3-0t64` not `libgtk-3-0`

### Verification

1. **Multi-method checks**: Packages, services, files, commands
2. **Specific verification**: Don't just check package existence
3. **Service validation**: Ensure services are active, not just installed
4. **File paths**: Verify actual binaries exist

### Debugging

1. **Preserve failed VMs**: Set `PRESERVE_VM_ON_FAILURE=1`
2. **Check serial console**: Boot diagnostics available
3. **Inspect cloud-init logs**: `/var/log/cloud-init-output.log`
4. **Verify network**: DHCP lease tracking helps identify IP issues

---

## Known Issues & Evolution Opportunities

### 🔴 CRITICAL: Reboot Reliability

**Symptom**: Some templates fail after reboot with "No route to host"  
**Impact**: `test-ubuntu24-ionchannel-piecewise`, `test-ubuntu24-fulldesktop-rustdesk`  
**Hypothesis**: Template-specific configuration differences, resource contention  
**Status**: Under investigation

**Workaround**: Use validated templates (treatment, rustdesk-only) as baseline

### 🟡 MEDIUM: Pop!_OS PPA Compatibility

**Symptom**: Pop!_OS PPA doesn't support Ubuntu 24.04 (noble)  
**Impact**: `ctrl-popos-cosmic-piecewise` template fails at PPA setup  
**Options**: Switch to 22.04 base, remove PPA, build from source  
**Status**: Evaluating options

---

## Metrics

### Code Quality

| Metric | Value | Status |
|--------|-------|--------|
| **Unsafe code** | 0 | ✅ Excellent |
| **Production mocks** | 0 | ✅ Excellent |
| **Hardcoded values** | Minimal | ✅ Good |
| **Tests passing** | All | ✅ 100% |

### Substrate Validation

| Metric | Value | Status |
|--------|-------|--------|
| **Templates** | 5 active | ✅ |
| **Validated** | 2 (40%) | 🟡 |
| **Successful checks** | 57/57 (100%) | ✅ |
| **Build time** | 4-7 min | ✅ |

---

## Philosophy: Primal Architecture

agentReagents embodies **primal philosophy**:

- **Self-Knowledge**: Components discover their own capabilities
- **Runtime Discovery**: DHCP, MAC-based, no hardcoding
- **Capability-Based**: No environment assumptions
- **Fractal/Isomorphic**: Patterns consistent across scales
- **Fast AND Safe**: Zero unsafe, zero-cost abstractions
- **Deep Debt Solutions**: Root causes, not bandaids

---

## Related Projects

- **[benchScale](../benchScale/)** - VM orchestration and substrate provisioning
- **[ionChannel](../ionChannel/)** - Remote desktop portal and A/B testing
- **[syntheticChemistry](../)** - Parent project and ecosystem

---

## Documentation

- [`ARCHITECTURE.md`](specs/ARCHITECTURE.md) - System architecture
- [`MANIFEST_SPEC.md`](specs/MANIFEST_SPEC.md) - Template specification
- [`GUIDANCE.md`](specs/GUIDANCE.md) - Best practices
- [`../STATUS.md`](../STATUS.md) - Project-wide status
- [`../INDEX.md`](../INDEX.md) - Complete documentation index

---

## Support

- **Issues**: Document in root `STATUS.md` and evolution docs
- **Templates**: See `templates/` for examples
- **Specs**: See `specs/` for detailed specifications

---

**agentReagents** - *Template-driven VM synthesis for modern infrastructure*

Production-ready • Evolution #23 validated • Zero unsafe code • Primal architecture

Made with 🦀 by the syntheticChemistry ecosystem
