# agentReagents - Developer Guidance

> **Best practices, patterns, and guidelines for building VMs with agentReagents**

## 🎯 Quick Start

### Installing agentReagents

```bash
# From source
cd agentReagents
cargo install --path .

# Check installation
agent-reagents --version
```

### Your First VM Build

```bash
# 1. Create a simple manifest
cat > my-first-vm.yaml <<EOF
name: my-first-vm
version: "1.0.0"
description: My first VM with agentReagents
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

vm_config:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 20

packages:
  - vim
  - git

users:
  - name: ubuntu
    ssh_authorized_keys:
      - "$(cat ~/.ssh/id_rsa.pub)"

verification:
  required_packages:
    - vim
    - git
EOF

# 2. Build the VM
agent-reagents build my-first-vm.yaml

# 3. SSH into your VM
# (IP address shown in build output)
ssh ubuntu@192.168.122.10
```

## 📚 Common Patterns

### Pattern 1: Development Environment

Create a full development environment with multiple languages:

```yaml
name: dev-env
version: "1.0.0"
description: Multi-language development environment
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

vm_config:
  memory_mb: 4096
  vcpus: 2
  disk_gb: 40

packages:
  # Base tools
  - build-essential
  - git
  - curl
  - wget
  
  # Languages
  - python3
  - python3-pip
  - nodejs
  - npm

users:
  - name: developer
    ssh_authorized_keys:
      - "ssh-rsa AAAA..."
    sudo: true

build_steps:
  # Install Rust
  - type: run_command
    command: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
  
  # Create workspace
  - type: run_command
    command: "mkdir -p /home/developer/workspace && chown developer:developer /home/developer/workspace"

verification:
  required_packages:
    - git
    - python3
    - nodejs
  
  required_commands:
    - name: "Rust installed"
      command: "rustc --version"
```

### Pattern 2: Web Server

Deploy a complete web server stack:

```yaml
name: web-server
version: "1.0.0"
description: Nginx + PostgreSQL + Redis web server
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

vm_config:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 30
  static_ip: "192.168.122.100"
  gateway: "192.168.122.1"
  netmask: "255.255.255.0"

packages:
  - nginx
  - postgresql
  - postgresql-contrib
  - redis-server
  - certbot
  - python3-certbot-nginx

build_steps:
  # Configure Nginx
  - type: write_file
    path: /etc/nginx/sites-available/default
    content: |
      server {
          listen 80 default_server;
          server_name _;
          
          location / {
              proxy_pass http://localhost:3000;
          }
      }
  
  # Enable and start services
  - type: run_command
    command: "systemctl enable nginx postgresql redis-server"
  
  - type: run_command
    command: "systemctl restart nginx"

users:
  - name: webadmin
    ssh_authorized_keys:
      - "ssh-rsa AAAA..."
    sudo: true

verification:
  required_packages:
    - nginx
    - postgresql
    - redis-server
  
  required_services:
    - nginx
    - postgresql
    - redis-server
  
  required_commands:
    - name: "Nginx config valid"
      command: "nginx -t"
    
    - name: "PostgreSQL running"
      command: "pg_isready"
  
  system_health:
    check_disk_space: true
    check_memory: true
    check_network: true
```

### Pattern 3: Desktop Environment

Create a full desktop VM for GUI applications:

```yaml
name: ubuntu-desktop
version: "1.0.0"
description: Ubuntu 24.04 with GNOME desktop
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

vm_config:
  memory_mb: 4096
  vcpus: 2
  disk_gb: 30

packages:
  # Desktop environment
  - ubuntu-desktop
  
  # Applications
  - firefox
  - thunderbird
  - libreoffice
  - gimp
  - vlc
  
  # Development tools
  - code      # VS Code
  - git
  - vim

users:
  - name: user
    ssh_authorized_keys:
      - "ssh-rsa AAAA..."
    sudo: true

build_steps:
  # Install RustDesk for remote access
  - type: run_command
    command: "wget https://github.com/rustdesk/rustdesk/releases/download/1.2.3/rustdesk-1.2.3-x86_64.deb"
  
  - type: run_command
    command: "dpkg -i rustdesk-1.2.3-x86_64.deb || apt-get install -f -y"

verification:
  required_packages:
    - ubuntu-desktop
    - firefox
    - code
  
  required_commands:
    - name: "Desktop installed"
      command: "which gnome-shell"
    
    - name: "Firefox installed"
      command: "firefox --version"
  
  required_services:
    - gdm
```

### Pattern 4: CI/CD Runner

Create a VM for CI/CD tasks:

```yaml
name: ci-runner
version: "1.0.0"
description: GitLab/GitHub CI runner with Docker
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

vm_config:
  memory_mb: 4096
  vcpus: 4
  disk_gb: 50

packages:
  # Container runtime
  - docker.io
  - docker-compose
  
  # Build tools
  - build-essential
  - git
  - curl
  - jq

users:
  - name: runner
    ssh_authorized_keys:
      - "ssh-rsa AAAA..."
    sudo: true

build_steps:
  # Add runner to docker group
  - type: run_command
    command: "usermod -aG docker runner"
  
  # Download GitLab Runner
  - type: run_command
    command: "curl -L https://packages.gitlab.com/install/repositories/runner/gitlab-runner/script.deb.sh | bash"
  
  - type: install_package
    package: gitlab-runner
  
  # Configure runner
  - type: write_file
    path: /home/runner/.gitlab-runner-setup.sh
    content: |
      #!/bin/bash
      # Register runner with: gitlab-runner register
      echo "Run: sudo gitlab-runner register"
    permissions: "0755"

verification:
  required_packages:
    - docker.io
    - gitlab-runner
  
  required_services:
    - docker
    - gitlab-runner
  
  required_commands:
    - name: "Docker works"
      command: "docker --version"
    
    - name: "Runner installed"
      command: "gitlab-runner --version"
```

## 🔧 CLI Usage

### Basic Commands

```bash
# Build a VM from manifest
agent-reagents build template.yaml

# Build with custom SSH key
agent-reagents build template.yaml \
    --ssh-key "ssh-rsa AAAA..."

# Build with verification disabled (not recommended)
agent-reagents build template.yaml --no-verify

# List available templates
agent-reagents list

# Show template details
agent-reagents info template-name

# Validate manifest without building
agent-reagents validate template.yaml
```

### Environment Variables

```bash
# Set default SSH key
export AGENT_REAGENTS_SSH_KEY="$(cat ~/.ssh/id_rsa.pub)"
agent-reagents build template.yaml

# Set default base image directory
export AGENT_REAGENTS_IMAGE_DIR="/custom/path/to/images"

# Set custom templates directory
export AGENT_REAGENTS_TEMPLATES_DIR="/custom/templates"
```

## 🚫 Common Pitfalls

### ❌ Don't: Forget SSH Keys

```bash
# BAD - No SSH key provided
agent-reagents build template.yaml

# GOOD - SSH key provided
agent-reagents build template.yaml \
    --ssh-key "$(cat ~/.ssh/id_rsa.pub)"
```

### ❌ Don't: Use Incorrect Base Image Path

```yaml
# BAD - Relative path
base_image: ubuntu-24.04.img

# GOOD - Absolute path
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img
```

### ❌ Don't: Skip Verification

```bash
# BAD - No verification
agent-reagents build template.yaml --no-verify

# GOOD - With verification (default)
agent-reagents build template.yaml
```

### ❌ Don't: Forget to Check Base Image Exists

```bash
# GOOD - Check before building
ls -lh /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

# If missing, download it
wget https://cloud-images.ubuntu.com/releases/24.04/release/ubuntu-24.04-server-cloudimg-amd64.img \
    -P /var/lib/libvirt/images/
```

## 🧪 Testing Manifests

### Test Locally

```bash
# 1. Validate manifest
agent-reagents validate my-template.yaml

# 2. Build in test mode
agent-reagents build my-template.yaml

# 3. SSH and verify
ssh ubuntu@$(agent-reagents info my-template | grep IP | awk '{print $2}')

# 4. Cleanup
virsh destroy my-template
virsh undefine my-template
```

### Test in CI/CD

```yaml
# .github/workflows/test-template.yml
name: Test Template

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v2
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build agent-reagents
        run: cd agentReagents && cargo build --release
      
      - name: Validate manifest
        run: ./target/release/agent-reagents validate templates/my-template.yaml
```

## 📖 Manifest Best Practices

### 1. Use Version Control

```bash
# Store manifests in git
git init templates
cd templates
git add *.yaml
git commit -m "Add VM templates"
```

### 2. Use Template Inheritance (Future)

```yaml
# base-template.yaml
name: base
vm_config:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 20

# specific-template.yaml (future)
extends: base-template.yaml
name: specific-vm
packages:
  - custom-package
```

### 3. Document Your Templates

```yaml
name: my-template
version: "1.0.0"
description: |
  Detailed description of what this template creates.
  
  Includes:
  - Ubuntu 24.04 base
  - GNOME desktop
  - Development tools
  
  Use cases:
  - Desktop development
  - GUI application testing
```

### 4. Use Meaningful Verification

```yaml
verification:
  # Verify critical components
  required_packages:
    - nginx
    - postgresql
  
  # Test actual functionality
  required_commands:
    - name: "Web server responding"
      command: "curl -f http://localhost"
    
    - name: "Database accessible"
      command: "pg_isready"
  
  # Check system health
  system_health:
    check_disk_space: true
    min_disk_mb: 1024
```

## 🔍 Debugging

### Enable Verbose Output

```bash
# Set log level
RUST_LOG=debug agent-reagents build template.yaml
```

### Check VM Status

```bash
# List VMs
virsh list --all

# Check specific VM
virsh dominfo my-vm

# View console
virsh console my-vm

# Check cloud-init logs
virsh console my-vm
# (login and check)
sudo cat /var/log/cloud-init.log
```

### Inspect Build Artifacts

```bash
# Check disk image
ls -lh /var/lib/libvirt/images/my-vm.qcow2

# Check cloud-init ISO
ls -lh /var/lib/libvirt/images/my-vm-cidata.iso

# Mount cloud-init ISO to inspect
sudo mount /var/lib/libvirt/images/my-vm-cidata.iso /mnt
cat /mnt/user-data
cat /mnt/meta-data
sudo umount /mnt
```

## 🚀 Performance Tips

### 1. Reuse Base Images

```bash
# Download once, use many times
wget https://cloud-images.ubuntu.com/releases/24.04/release/ubuntu-24.04-server-cloudimg-amd64.img \
    -P /var/lib/libvirt/images/

# Build multiple VMs from same base
agent-reagents build template1.yaml
agent-reagents build template2.yaml
agent-reagents build template3.yaml
```

### 2. Optimize Package Installation

```yaml
# Group related packages
packages:
  # Base tools (fast install)
  - curl
  - wget
  - vim
  
  # Large packages (slower)
  - ubuntu-desktop
  - libreoffice
```

### 3. Use Appropriate Resources

```yaml
# Small VM for testing
vm_config:
  memory_mb: 1024
  vcpus: 1
  disk_gb: 10

# Large VM for development
vm_config:
  memory_mb: 8192
  vcpus: 4
  disk_gb: 50
```

## 📚 Further Reading

- **[OVERVIEW.md](OVERVIEW.md)** - Project overview
- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Internal architecture
- **[MANIFEST_SPEC.md](MANIFEST_SPEC.md)** - Complete manifest reference
- **[../README.md](../README.md)** - Getting started guide

## 🤝 Contributing Templates

Want to share your templates?

1. **Create high-quality template**
   - Clear documentation
   - Comprehensive verification
   - Tested and working

2. **Submit PR**
   - Add to `templates/` directory
   - Include README with use cases
   - Add to template registry

3. **Follow conventions**
   - Use semantic versioning
   - Include descriptions
   - Add verification checks

---

**Happy building with agentReagents!** 🦀✨

