# agentReagents - Architecture

> **Internal architecture and design decisions for declarative VM builds**

## 🏛️ System Architecture

### High-Level Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Layer                               │
│          (agent-reagents binary, argument parsing)              │
└────────────────────┬────────────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────────────────┐
│                  Template Layer                                 │
│  ┌──────────────────┐        ┌──────────────────┐              │
│  │  TemplateManifest│◄───────┤TemplateRegistry │              │
│  │  (YAML parsing)  │        │  (Discovery)     │              │
│  └──────────┬───────┘        └──────────────────┘              │
└─────────────┼──────────────────────────────────────────────────┘
              │
┌─────────────▼──────────────────────────────────────────────────┐
│                   Builder Layer                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ImageBuilder  │  │  Executor    │  │ Verification │✨        │
│  │(Orchestration│──┤  (Steps)     │──┤  (Checks)    │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│  ┌──────────────┐  ┌──────────────┐                            │
│  │  VmHandle    │  │    State     │                            │
│  │ (VM access)  │  │  (Tracking)  │                            │
│  └──────────────┘  └──────────────┘                            │
└─────────────┬──────────────────────────────────────────────────┘
              │
┌─────────────▼──────────────────────────────────────────────────┐
│                 benchScale Layer                                │
│  (LibvirtBackend, CloudInit, VM lifecycle)                      │
└─────────────────────────────────────────────────────────────────┘
```

## 📦 Module Architecture

### 1. Template Layer

**Purpose:** Parse, validate, and manage YAML manifests.

#### TemplateManifest (`templates/manifest.rs`)

```rust
pub struct TemplateManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub base_image: PathBuf,
    pub vm_config: VmConfig,
    pub packages: Vec<String>,
    pub users: Vec<UserConfig>,
    pub build_steps: Vec<BuildStep>,
    pub verification: VerificationConfig,
}
```

**Features:**
- **Serde deserialization** - Type-safe YAML parsing
- **Validation** - Ensure manifest is well-formed
- **Defaults** - Sensible defaults for optional fields
- **Error reporting** - Clear error messages for invalid manifests

#### TemplateRegistry (`templates/registry.rs`)

```rust
pub struct TemplateRegistry {
    templates_dir: PathBuf,
    templates: HashMap<String, TemplateInfo>,
}

impl TemplateRegistry {
    pub fn discover_templates(&mut self) -> Result<()>;
    pub fn get_template(&self, name: &str) -> Option<&TemplateInfo>;
    pub fn list_templates(&self) -> Vec<&TemplateInfo>;
}
```

**Features:**
- **Template discovery** - Scan directories for manifests
- **Checksum validation** - Ensure template integrity
- **Caching** - Fast repeated access
- **Version management** - Track template versions

### 2. Builder Layer

**Purpose:** Orchestrate VM builds from manifests.

#### ImageBuilder (`builder/mod.rs`)

```rust
pub struct ImageBuilder {
    manifest: TemplateManifest,
    backend: LibvirtBackend,
    state: BuildState,
}

impl ImageBuilder {
    pub async fn build(&mut self) -> Result<VmHandle>;
    pub async fn build_and_verify(&mut self) -> Result<VmHandle>;
    async fn create_vm(&mut self) -> Result<VmHandle>;
    async fn provision_vm(&mut self, vm: &VmHandle) -> Result<()>;
    async fn verify_vm(&mut self, vm: &VmHandle) -> Result<VerificationResult>;
}
```

**Responsibilities:**
1. Convert manifest to benchScale CloudInit
2. Create VM using benchScale backend
3. Monitor provisioning progress
4. Execute build steps
5. Run verification checks
6. Report results

#### Executor (`builder/executor.rs`)

```rust
pub async fn execute_build_steps(
    vm: &VmHandle,
    steps: &[BuildStep]
) -> Result<()>;

async fn execute_build_step(
    vm: &VmHandle,
    step: &BuildStep
) -> Result<()>;
```

**Build Step Types:**
- `RunCommand` - Execute shell command
- `WriteFile` - Create file with content
- `InstallPackage` - Install additional package
- `WaitForService` - Wait for service to start

#### Verification System ✨ (`builder/verification.rs`)

```rust
pub struct VerificationResult {
    pub checks: Vec<VerificationCheck>,
    pub total: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub passed: bool,
}

pub async fn verify_installation(
    vm: &VmHandle,
    manifest: &TemplateManifest
) -> Result<VerificationResult>;
```

**Verification Types:**
1. **Package Verification** - Check packages installed
2. **Service Verification** - Check services running
3. **File Verification** - Check files exist
4. **Command Verification** - Execute test commands
5. **System Health** - Check disk, memory, network
6. **Desktop Detection** - Verify desktop environment

#### VmHandle (`builder/vm_handle.rs`)

```rust
pub struct VmHandle {
    backend: LibvirtBackend,
    node: NodeInfo,
}

impl VmHandle {
    pub async fn ssh_exec(&self, user: &str, cmd: &str) -> Result<String>;
    pub fn ip_address(&self) -> &str;
    pub fn name(&self) -> &str;
    pub fn backend(&self) -> &LibvirtBackend;
    pub fn node(&self) -> &NodeInfo;
}
```

**Purpose:** Abstraction over VM access for testing and operations.

#### State Management (`builder/state.rs`)

```rust
pub enum BuildState {
    Idle,
    Starting,
    CreatingVm,
    WaitingForBoot,
    WaitingForCloudInit,
    ExecutingSteps,
    Verifying,
    Complete,
    Failed(String),
}
```

**State Transitions:**
```
Idle → Starting → CreatingVm → WaitingForBoot → 
WaitingForCloudInit → ExecutingSteps → Verifying → Complete
                                                    ↓
                                                 Failed
```

## 🔄 Data Flow

### Complete Build Flow

```
1. CLI parses arguments
   ↓
2. Load YAML manifest
   ↓
3. Validate manifest structure
   ↓
4. Create ImageBuilder
   ↓
5. Convert manifest → CloudInit config
   ↓
6. Backend creates VM (benchScale)
   ↓
7. Wait for VM boot
   ↓
8. Monitor cloud-init status
   ↓
9. Execute build steps
   ↓
10. Run verification checks
    ↓
11. Report results
    ↓
12. Return VmHandle or error
```

### Verification Flow

```
1. Parse verification config from manifest
   ↓
2. Run package checks (dpkg -l)
   ↓
3. Run service checks (systemctl status)
   ↓
4. Run file checks (test -f)
   ↓
5. Run command checks (execute & check exit code)
   ↓
6. Run system health checks (df, free, ping)
   ↓
7. Aggregate results
   ↓
8. Generate verification report
   ↓
9. Return pass/fail
```

## 🧩 Component Interactions

### ImageBuilder ↔ TemplateManifest

```rust
impl ImageBuilder {
    pub fn new(manifest: TemplateManifest) -> Self {
        // Convert manifest to internal representation
        Self {
            manifest,
            backend: LibvirtBackend::new()?,
            state: BuildState::Idle,
        }
    }
}
```

### ImageBuilder ↔ benchScale

```rust
// Convert manifest to CloudInit
let cloud_init = CloudInit::builder()
    .add_user(&manifest.user, &ssh_key)
    .packages(manifest.packages.iter())
    .build();

// Create VM using benchScale
let node = self.backend.create_desktop_vm(
    &manifest.name,
    &manifest.base_image,
    &cloud_init,
    manifest.vm_config.memory_mb,
    manifest.vm_config.vcpus,
    manifest.vm_config.disk_gb,
).await?;
```

### Verification ↔ VmHandle

```rust
// Execute verification checks via SSH
async fn verify_packages(
    vm: &VmHandle,
    manifest: &TemplateManifest
) -> Result<Vec<VerificationCheck>> {
    let mut checks = Vec::new();
    
    for package in &manifest.verification.required_packages {
        let result = vm.ssh_exec(
            "ubuntu",
            &format!("dpkg -l {} 2>/dev/null | grep -q '^ii'", package)
        ).await;
        
        checks.push(VerificationCheck {
            name: format!("Package: {}", package),
            passed: result.is_ok(),
            details: result.err().map(|e| e.to_string()),
        });
    }
    
    Ok(checks)
}
```

## 📊 State Management

### Build State Machine

```
┌──────┐
│ Idle │
└───┬──┘
    │ build()
    ▼
┌──────────┐
│ Starting │
└────┬─────┘
     │
     ▼
┌────────────┐
│CreatingVm  │◄───┐
└────┬───────┘    │ retry
     │            │
     ▼            │
┌────────────────┐│
│WaitingForBoot  ├┘
└────┬───────────┘
     │
     ▼
┌───────────────────┐
│WaitingForCloudInit│
└────┬──────────────┘
     │
     ▼
┌──────────────────┐
│ExecutingSteps    │
└────┬─────────────┘
     │
     ▼
┌──────────┐
│Verifying │
└────┬─────┘
     │
     ├─────────┬─────────┐
     ▼         ▼         ▼
┌────────┐ ┌────────┐ ┌────────┐
│Complete│ │ Failed │ │Terminal│
└────────┘ └────────┘ └────────┘
```

## 🔐 Safety & Type Safety

### Type-Safe Manifest Parsing

```rust
// Serde ensures type safety at deserialization
#[derive(Debug, Deserialize)]
pub struct TemplateManifest {
    pub name: String,        // Must be string
    pub version: String,     // Must be string
    pub vm_config: VmConfig, // Must be valid VmConfig
    // ...
}

#[derive(Debug, Deserialize)]
pub struct VmConfig {
    pub memory_mb: u32,  // Must be positive integer
    pub vcpus: u32,      // Must be positive integer
    pub disk_gb: u32,    // Must be positive integer
}
```

### Error Handling

```rust
// All errors propagated with anyhow::Result
pub async fn build(&mut self) -> Result<VmHandle> {
    self.create_vm()
        .await
        .context("Failed to create VM")?;
    
    self.provision_vm()
        .await
        .context("Failed to provision VM")?;
    
    Ok(vm_handle)
}
```

### Resource Safety

```rust
// VmHandle cleanup on drop (future enhancement)
impl Drop for VmHandle {
    fn drop(&mut self) {
        // Optionally cleanup VM resources
        // Currently manual via backend.delete_node()
    }
}
```

## 🚀 Performance Characteristics

### Build Times

**Typical build (Ubuntu Desktop):**
- Manifest parsing: <10ms
- VM creation: ~400ms (image copy)
- Boot time: ~30s
- Cloud-init: ~2-5 minutes (depends on packages)
- Verification: ~10-30s

**Total: 3-6 minutes for full build**

### Optimization Opportunities

1. **Parallel package installation**
   - Currently serial via cloud-init
   - Could use custom apt commands with parallel downloads

2. **Template caching**
   - Cache parsed manifests
   - Cache verification results
   - Reuse base images

3. **Incremental builds**
   - Detect changes in manifest
   - Only rebuild changed layers
   - Snapshot-based builds

## 🧪 Testing Strategy

### Unit Tests

```rust
#[test]
fn test_manifest_parsing() {
    let yaml = r#"
name: test
version: "1.0.0"
base_image: /path/to/image
vm_config:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 20
"#;
    
    let manifest: TemplateManifest = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(manifest.name, "test");
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_build() {
    let manifest = TemplateManifest::from_file("test.yaml")?;
    let mut builder = ImageBuilder::new(manifest);
    
    let vm = builder.build().await?;
    
    assert!(vm.ip_address().starts_with("192.168"));
    
    vm.backend().delete_node(vm.name()).await?;
}
```

### Verification Tests

```rust
#[test]
fn test_verification_result() {
    let checks = vec![
        VerificationCheck {
            name: "Package: vim".to_string(),
            passed: true,
            details: None,
        },
    ];
    
    let result = VerificationResult::new(checks);
    
    assert_eq!(result.total, 1);
    assert_eq!(result.passed_count, 1);
    assert!(result.passed);
}
```

## 📈 Evolution Plan

### Phase 1: Template Enhancements
- Template inheritance (base + overrides)
- Template variables (${{VAR}} substitution)
- Conditional sections

### Phase 2: Multi-Platform Support
- Fedora/RHEL (yum/dnf)
- Arch Linux (pacman)
- Alpine (apk)
- Container images (Docker, Podman)

### Phase 3: Advanced Verification
- Performance tests (CPU, memory, disk I/O)
- Security scans (CVE detection)
- Compliance checks (CIS benchmarks)

### Phase 4: Collaboration Features
- Template marketplace
- Template versioning (git integration)
- Template sharing and discovery

---

**Current Status:** Production Ready with clear evolution path 🦀✨

