# agentReagents - Project Overview

> **Declarative VM builds from YAML manifests with automated verification**

## 🎯 Purpose

agentReagents is a production-ready Rust CLI tool that enables declarative VM builds from YAML manifests. It provides automated VM provisioning, package installation, and comprehensive verification, making it easy to create reproducible VM templates for testing and development.

## 🏗️ Core Capabilities

### 1. Declarative VM Builds
- **YAML manifests** - Define VMs with simple configuration files
- **Template management** - Version-controlled VM definitions
- **Reproducible builds** - Same manifest = same VM every time
- **Type-safe validation** - Catch errors before deployment

### 2. Automated Provisioning
- **Cloud-init integration** - Automated VM configuration
- **Package installation** - Declare packages in manifest
- **User management** - SSH keys injected automatically
- **Network configuration** - Static IPs and networking

### 3. Comprehensive Verification
- **Package verification** - Confirm all packages installed
- **Service verification** - Check services running
- **File verification** - Validate required files exist
- **Command verification** - Test custom commands
- **System health** - Comprehensive health checks
- **Desktop detection** - Verify desktop environment

### 4. Template Registry
- **Template discovery** - Find available templates
- **Checksum validation** - Ensure template integrity
- **Template caching** - Fast subsequent builds
- **Version management** - Track template versions

## 📐 Architecture

```
agentReagents/
├── src/
│   ├── builder/           # VM build orchestration
│   │   ├── mod.rs         # Build coordinator
│   │   ├── executor.rs    # Build step execution
│   │   ├── verification.rs # Verification system ✨
│   │   ├── vm_handle.rs   # VM handle abstraction
│   │   └── state.rs       # Build state management
│   │
│   ├── templates/         # Template management
│   │   ├── manifest.rs    # YAML manifest parsing
│   │   └── registry.rs    # Template registry
│   │
│   ├── bin/               # CLI binaries
│   │   └── agent-reagents.rs # Main CLI
│   │
│   └── lib.rs             # Library root
│
├── specs/                 # Technical specifications
│   ├── OVERVIEW.md        # This file
│   ├── ARCHITECTURE.md    # Detailed architecture
│   ├── MANIFEST_SPEC.md   # Manifest format spec
│   └── GUIDANCE.md        # Developer guidance
│
├── templates/             # VM manifests
│   ├── ubuntu24-minimal-baseline.yaml
│   ├── popos-24-cosmic.yaml
│   ├── lithoSpore-validation.yaml
│   └── gates/             # Gate provisioning templates
│
└── README.md              # Getting started guide
```

## 🔑 Key Design Principles

### 1. **Declarative Over Imperative**
- Define desired state, not steps
- Idempotent operations
- Reproducible results

### 2. **Type Safety**
- Strong typing for manifest structures
- Compile-time validation
- Serde for safe deserialization

### 3. **Verification First**
- Comprehensive verification system
- Automated validation
- Clear pass/fail reporting

### 4. **Standalone + Network Effect**
- Useful on its own for VM builds
- More powerful with benchScale
- Composable with other tools

### 5. **User Friendly**
- Simple YAML manifests
- Clear error messages
- Helpful CLI output

## 🚀 Usage Philosophy

### Standalone Tool

Simple VM builds from manifests:

```bash
# Build a VM from a manifest
agent-reagents build templates/ubuntu24-minimal-baseline.yaml \
    --ssh-key "$(cat ~/.ssh/id_ed25519.pub)"

# VM is created, provisioned, and verified automatically
```

### With benchScale

Programmatic VM builds:

```rust
use agent_reagents::{TemplateManifest, ImageBuilder};
use benchscale::LibvirtBackend;

let manifest = TemplateManifest::from_file("template.yaml")?;
let backend = LibvirtBackend::new()?;

let builder = ImageBuilder::new(manifest, backend);
let vm = builder.build().await?;
```

### Network Effect

- **CI/CD pipelines**: Automated VM provisioning
- **Testing frameworks**: Reproducible test environments
- **Development**: Consistent dev environments
- **Infrastructure as Code**: Version-controlled VMs

## 📋 Manifest Format

### Basic Example

```yaml
name: ubuntu-desktop
version: "1.0.0"
description: Ubuntu 24.04 with desktop environment
base_image: /var/lib/libvirt/images/ubuntu-24.04.img

vm_config:
  memory_mb: 4096
  vcpus: 2
  disk_gb: 30

packages:
  - ubuntu-desktop
  - firefox
  - vim
  - git

users:
  - name: ubuntu
    ssh_authorized_keys:
      - "ssh-rsa AAAA..."

verification:
  required_packages:
    - ubuntu-desktop
    - firefox
  
  required_files:
    - /usr/bin/firefox
    - /usr/bin/git
  
  required_commands:
    - name: "Check Firefox"
      command: "firefox --version"
```

### Advanced Example

```yaml
name: development-workstation
version: "2.0.0"
description: Full development environment
base_image: /var/lib/libvirt/images/ubuntu-24.04.img

vm_config:
  memory_mb: 8192
  vcpus: 4
  disk_gb: 50
  static_ip: "192.168.122.50"
  gateway: "192.168.122.1"
  netmask: "255.255.255.0"

packages:
  # Development tools
  - build-essential
  - cmake
  - git
  - curl
  - wget
  
  # Languages
  - rustc
  - cargo
  - python3
  - python3-pip
  - nodejs
  - npm
  
  # Editors
  - vim
  - neovim
  - code  # VS Code

users:
  - name: developer
    ssh_authorized_keys:
      - "ssh-rsa AAAA..."
    sudo: true
  
  - name: tester
    ssh_authorized_keys:
      - "ssh-rsa BBBB..."

build_steps:
  - type: run_command
    command: "rustup default stable"
  
  - type: run_command
    command: "pip3 install pytest black mypy"
  
  - type: write_file
    path: /etc/motd
    content: "Development Workstation v2.0"

verification:
  required_packages:
    - rustc
    - cargo
    - python3
    - nodejs
  
  required_commands:
    - name: "Rust version"
      command: "rustc --version"
    
    - name: "Python version"
      command: "python3 --version"
    
    - name: "Node version"
      command: "node --version"
  
  system_health:
    check_disk_space: true
    check_memory: true
    check_network: true
```

## 📊 Current Status

| Metric | Value | Target |
|--------|-------|--------|
| **Production Ready** | ✅ Yes | ✅ |
| **Test Coverage** | 13 tests passing | 90%+ with llvm-cov |
| **Code Quality** | 88% | 95% |
| **Unsafe Code** | 0 blocks | 0 |
| **Lines of Code** | 2,656 | <5,000 |

## 🎯 Features

### ✅ Implemented

- ✅ **YAML manifest parsing** with type safety
- ✅ **Template registry** for discovery and management
- ✅ **Automated VM builds** from manifests
- ✅ **Cloud-init integration** for provisioning
- ✅ **Comprehensive verification** system
- ✅ **Package verification** (dpkg-based)
- ✅ **Service verification** (systemd)
- ✅ **File verification** (existence checks)
- ✅ **Command verification** (execution checks)
- ✅ **System health checks** (disk, memory, network)
- ✅ **Desktop environment detection**
- ✅ **CLI with clear output**
- ✅ **Error handling** with helpful messages

### 📋 Planned Enhancements

- [ ] **Coverage measurement** with llvm-cov (90% target)
- [ ] **More verification types** (network tests, performance tests)
- [ ] **Template inheritance** (base templates + overrides)
- [ ] **Multi-distribution support** (Fedora, Arch, etc.)
- [ ] **Container builds** (Docker, Podman)
- [ ] **Parallel builds** for multiple VMs
- [ ] **Interactive mode** for template creation
- [ ] **Template marketplace** for sharing templates

## 🔄 Workflow

### 1. Create Manifest

```bash
# Create a new manifest
cat > my-template.yaml <<EOF
name: my-vm
version: "1.0.0"
description: My custom VM
base_image: /var/lib/libvirt/images/ubuntu-24.04.img

vm_config:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 20

packages:
  - vim
  - git

verification:
  required_packages:
    - vim
    - git
EOF
```

### 2. Build VM

```bash
# Build the VM
agent-reagents build my-template.yaml \
    --ssh-key "$(cat ~/.ssh/id_rsa.pub)"
```

### 3. Automatic Process

The tool automatically:
1. Validates manifest
2. Creates VM from base image
3. Injects SSH keys
4. Installs packages
5. Runs custom commands
6. Verifies installation
7. Reports results

### 4. Result

```
✅ VM created successfully
✅ All packages installed
✅ All services running
✅ All files present
✅ All commands passed
✅ System health OK

VM Details:
  Name: my-vm
  IP: 192.168.122.10
  SSH: ssh ubuntu@192.168.122.10
```

## 🤝 Integration Examples

### With benchScale (Programmatic)

```rust
use agent_reagents::{TemplateManifest, ImageBuilder};
use benchscale::LibvirtBackend;

let manifest = TemplateManifest::from_file("template.yaml")?;
let backend = LibvirtBackend::new()?;

let builder = ImageBuilder::new(manifest, backend);
let vm = builder.build_and_verify().await?;

println!("VM ready: {} at {}", vm.name(), vm.ip_address());
```

### With CI/CD (GitHub Actions)

```yaml
name: Test on Fresh VM
on: [push]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Install agent-reagents
        run: cargo install agent-reagents
      
      - name: Create test VM
        run: |
          agent-reagents build test-env.yaml \
            --ssh-key "${{ secrets.SSH_KEY }}"
      
      - name: Run tests in VM
        run: ssh ubuntu@test-vm "cd /project && cargo test"
```

### With Docker (Containerized Builds)

```dockerfile
FROM rust:latest

RUN cargo install agent-reagents

COPY templates/ /templates/

ENTRYPOINT ["agent-reagents", "build"]
CMD ["/templates/default.yaml"]
```

## 🎓 Learning Path

### Beginner

1. Read [GUIDANCE.md](GUIDANCE.md) for basic usage
2. Try example templates in `templates/`
3. Create simple custom template
4. Build first VM

### Intermediate

1. Read [MANIFEST_SPEC.md](MANIFEST_SPEC.md) for full format
2. Create complex multi-package template
3. Add custom verification checks
4. Integrate with CI/CD

### Advanced

1. Read [ARCHITECTURE.md](ARCHITECTURE.md) for internals
2. Extend verification system
3. Add custom build steps
4. Contribute to project

## 📚 Documentation

- **[GUIDANCE.md](GUIDANCE.md)** - Developer guidance and patterns
- **[MANIFEST_SPEC.md](MANIFEST_SPEC.md)** - Complete manifest specification
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Internal architecture
- **[README.md](../README.md)** - Getting started

## 🌟 Key Differentiators

✅ **Declarative** - YAML manifests, not scripts  
✅ **Type-Safe** - Compile-time validation  
✅ **Comprehensive Verification** - Automated checks  
✅ **Standalone + Composable** - Works alone or with benchScale  
✅ **Production Ready** - 13 tests, 0 unsafe code  
✅ **Modern Rust** - Async, idiomatic, safe  

---

**agentReagents: Declarative VMs made simple** 🦀✨

