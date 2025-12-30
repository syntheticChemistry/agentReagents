# agentReagents - Manifest Specification

> **Complete YAML manifest format reference**

## 📋 Overview

Template manifests are YAML files that define VM configurations, packages, users, build steps, and verification checks. They enable declarative, reproducible VM builds.

## 🔑 Required Fields

### Basic Information

```yaml
name: string              # VM name (must be unique)
version: string           # Semantic version (e.g., "1.0.0")
description: string       # Human-readable description
base_image: path          # Path to base cloud image
```

**Example:**

```yaml
name: ubuntu-desktop
version: "1.0.0"
description: Ubuntu 24.04 with GNOME desktop
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img
```

### VM Configuration

```yaml
vm_config:
  memory_mb: integer      # Memory in megabytes (1024-32768)
  vcpus: integer          # Number of vCPUs (1-16)
  disk_gb: integer        # Disk size in gigabytes (10-500)
```

**Example:**

```yaml
vm_config:
  memory_mb: 4096         # 4GB RAM
  vcpus: 2                # 2 vCPUs
  disk_gb: 30             # 30GB disk
```

## 📦 Optional Fields

### Packages

Array of package names to install via package manager:

```yaml
packages:
  - package1
  - package2
  - package3
```

**Example:**

```yaml
packages:
  # Desktop environment
  - ubuntu-desktop
  
  # Development tools
  - git
  - vim
  - curl
  - wget
  
  # Applications
  - firefox
  - thunderbird
```

### Users

Array of user configurations:

```yaml
users:
  - name: string                    # Username
    ssh_authorized_keys:            # SSH public keys
      - "ssh-rsa ..."
    sudo: boolean                   # Sudo access (optional, default: false)
    shell: string                   # Login shell (optional, default: /bin/bash)
```

**Example:**

```yaml
users:
  - name: ubuntu
    ssh_authorized_keys:
      - "ssh-rsa AAAAB3NzaC1yc2EAAA... user@host"
    sudo: true
    shell: /bin/bash
  
  - name: developer
    ssh_authorized_keys:
      - "ssh-rsa AAAAB3NzaC1yc2EAAA... dev@host"
    sudo: true
  
  - name: tester
    ssh_authorized_keys:
      - "ssh-rsa AAAAB3NzaC1yc2EAAA... test@host"
    sudo: false
```

### Network Configuration

Configure static networking:

```yaml
vm_config:
  # ... other config ...
  static_ip: string       # Static IP address (optional)
  gateway: string         # Gateway IP (required if static_ip)
  netmask: string         # Netmask (required if static_ip)
  dns_servers:            # DNS servers (optional)
    - string
```

**Example:**

```yaml
vm_config:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 20
  static_ip: "192.168.122.50"
  gateway: "192.168.122.1"
  netmask: "255.255.255.0"
  dns_servers:
    - "8.8.8.8"
    - "8.8.4.4"
```

### Build Steps

Custom commands to run during build:

```yaml
build_steps:
  - type: run_command
    command: string
    timeout: integer      # Timeout in seconds (optional)
  
  - type: write_file
    path: string
    content: string
    permissions: string   # Unix permissions (optional, default: "0644")
  
  - type: install_package
    package: string
  
  - type: wait_for_service
    service: string
    timeout: integer      # Timeout in seconds (optional)
```

**Example:**

```yaml
build_steps:
  # Install Rust toolchain
  - type: run_command
    command: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
    timeout: 300
  
  # Configure Rust
  - type: run_command
    command: "source $HOME/.cargo/env && rustup default stable"
  
  # Create welcome message
  - type: write_file
    path: /etc/motd
    content: |
      ╔═══════════════════════════════════╗
      ║  Development Environment v1.0.0   ║
      ╚═══════════════════════════════════╝
    permissions: "0644"
  
  # Install additional tool
  - type: install_package
    package: htop
  
  # Wait for Docker
  - type: wait_for_service
    service: docker
    timeout: 60
```

### Verification

Define checks to validate the build:

```yaml
verification:
  required_packages:      # Packages that must be installed
    - string
  
  required_files:         # Files that must exist
    - string
  
  required_commands:      # Commands to test
    - name: string        # Check name
      command: string     # Command to run
      expect_exit_code: integer  # Expected exit code (optional, default: 0)
  
  required_services:      # Services that must be running
    - string
  
  system_health:
    check_disk_space: boolean     # Check disk space (optional)
    check_memory: boolean         # Check memory (optional)
    check_network: boolean        # Check network (optional)
    min_disk_mb: integer          # Min free disk MB (optional)
    min_memory_mb: integer        # Min free memory MB (optional)
```

**Example:**

```yaml
verification:
  required_packages:
    - ubuntu-desktop
    - firefox
    - git
    - vim
  
  required_files:
    - /usr/bin/firefox
    - /usr/bin/git
    - /usr/bin/vim
    - /etc/motd
  
  required_commands:
    - name: "Check Firefox version"
      command: "firefox --version"
      expect_exit_code: 0
    
    - name: "Check Git installation"
      command: "git --version"
    
    - name: "Check Rust toolchain"
      command: "rustc --version"
  
  required_services:
    - gdm           # GNOME Display Manager
    - networkmanager
  
  system_health:
    check_disk_space: true
    check_memory: true
    check_network: true
    min_disk_mb: 1024      # At least 1GB free
    min_memory_mb: 512     # At least 512MB free
```

## 📖 Complete Examples

### Example 1: Basic Ubuntu Desktop

```yaml
name: ubuntu-24-04-desktop
version: "1.0.0"
description: Ubuntu 24.04 with GNOME desktop environment
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

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
      - "ssh-rsa AAAAB3NzaC1yc2EAAA..."
    sudo: true

verification:
  required_packages:
    - ubuntu-desktop
    - firefox
  
  required_commands:
    - name: "Check desktop environment"
      command: "which gnome-shell"
```

### Example 2: Pop!_OS with COSMIC

```yaml
name: popos-24-cosmic
version: "1.0.0"
description: Pop!_OS 24.04 with COSMIC desktop environment
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

vm_config:
  memory_mb: 4096
  vcpus: 2
  disk_gb: 30

packages:
  # Add Pop!_OS repository packages
  - software-properties-common
  - wget
  - curl
  - git

build_steps:
  # Add COSMIC repository
  - type: run_command
    command: "add-apt-repository -y ppa:system76/cosmic"
  
  # Update package list
  - type: run_command
    command: "apt-get update"
  
  # Install COSMIC
  - type: run_command
    command: "apt-get install -y cosmic-session cosmic-comp"
    timeout: 1800  # 30 minutes for large download

users:
  - name: ubuntu
    ssh_authorized_keys:
      - "ssh-rsa AAAAB3NzaC1yc2EAAA..."

verification:
  required_packages:
    - cosmic-session
    - cosmic-comp
  
  required_commands:
    - name: "Check COSMIC installation"
      command: "dpkg -l | grep cosmic"
```

### Example 3: Development Workstation

```yaml
name: dev-workstation
version: "2.0.0"
description: Complete development environment with multiple languages
base_image: /var/lib/libvirt/images/ubuntu-24.04-server-cloudimg-amd64.img

vm_config:
  memory_mb: 8192
  vcpus: 4
  disk_gb: 50
  static_ip: "192.168.122.100"
  gateway: "192.168.122.1"
  netmask: "255.255.255.0"

packages:
  # Base development tools
  - build-essential
  - cmake
  - git
  - curl
  - wget
  - htop
  - tree
  
  # Languages
  - python3
  - python3-pip
  - nodejs
  - npm
  
  # Editors
  - vim
  - neovim
  - emacs
  
  # Containers
  - docker.io
  - docker-compose

users:
  - name: developer
    ssh_authorized_keys:
      - "ssh-rsa AAAAB3NzaC1yc2EAAA..."
    sudo: true
  
  - name: tester
    ssh_authorized_keys:
      - "ssh-rsa AAAAB3NzaC1yc2EAAA..."
    sudo: false

build_steps:
  # Install Rust
  - type: run_command
    command: "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
  
  # Configure Rust
  - type: run_command
    command: "echo 'source $HOME/.cargo/env' >> ~/.bashrc"
  
  # Install Python packages
  - type: run_command
    command: "pip3 install pytest black mypy flake8"
  
  # Install Node packages
  - type: run_command
    command: "npm install -g typescript eslint prettier"
  
  # Add developer to docker group
  - type: run_command
    command: "usermod -aG docker developer"
  
  # Create workspace directory
  - type: run_command
    command: "mkdir -p /home/developer/workspace"
  
  # Create MOTD
  - type: write_file
    path: /etc/motd
    content: |
      ╔═══════════════════════════════════════════════╗
      ║  Development Workstation v2.0.0               ║
      ║                                               ║
      ║  Languages: Rust, Python, Node.js             ║
      ║  Tools: Git, Docker, Vim, Neovim              ║
      ║  Workspace: /home/developer/workspace         ║
      ╚═══════════════════════════════════════════════╝

verification:
  required_packages:
    - build-essential
    - git
    - python3
    - nodejs
    - docker.io
  
  required_files:
    - /usr/bin/git
    - /usr/bin/python3
    - /usr/bin/node
    - /usr/bin/docker
    - /home/developer/workspace
  
  required_commands:
    - name: "Check Rust installation"
      command: "rustc --version"
    
    - name: "Check Python installation"
      command: "python3 --version"
    
    - name: "Check Node installation"
      command: "node --version"
    
    - name: "Check Docker installation"
      command: "docker --version"
    
    - name: "Check pytest"
      command: "pytest --version"
  
  required_services:
    - docker
  
  system_health:
    check_disk_space: true
    check_memory: true
    check_network: true
    min_disk_mb: 5120      # At least 5GB free
    min_memory_mb: 1024    # At least 1GB free
```

## 🔍 Validation Rules

### Name
- Must be lowercase
- Can contain hyphens and numbers
- Must start with a letter
- Max 64 characters

### Version
- Must follow semantic versioning (MAJOR.MINOR.PATCH)
- Examples: "1.0.0", "2.1.3", "0.1.0-beta"

### Memory
- Minimum: 1024 MB (1GB)
- Maximum: 32768 MB (32GB)
- Must be power of 2 or multiple of 1024

### vCPUs
- Minimum: 1
- Maximum: 16
- Must not exceed host CPU count

### Disk
- Minimum: 10 GB
- Maximum: 500 GB

### IP Addresses
- Must be valid IPv4 addresses
- Static IP must be in private range (10.x, 172.16-31.x, 192.168.x)

## ⚠️ Common Pitfalls

### 1. Forgetting Required Fields

```yaml
# ❌ BAD - Missing base_image
name: my-vm
version: "1.0.0"

# ✅ GOOD
name: my-vm
version: "1.0.0"
base_image: /path/to/image.img
vm_config:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 20
```

### 2. Invalid YAML Syntax

```yaml
# ❌ BAD - Inconsistent indentation
packages:
  - vim
   - git   # Wrong indentation

# ✅ GOOD
packages:
  - vim
  - git
```

### 3. Missing Gateway with Static IP

```yaml
# ❌ BAD - Static IP without gateway
vm_config:
  static_ip: "192.168.122.50"

# ✅ GOOD
vm_config:
  static_ip: "192.168.122.50"
  gateway: "192.168.122.1"
  netmask: "255.255.255.0"
```

## 📚 Best Practices

### 1. Use Meaningful Names

```yaml
# ❌ BAD
name: vm1

# ✅ GOOD
name: ubuntu-desktop-dev
```

### 2. Add Descriptions

```yaml
# ✅ GOOD
name: web-server
version: "1.0.0"
description: Ubuntu 24.04 with Nginx, PostgreSQL, and Redis for web applications
```

### 3. Group Related Packages

```yaml
packages:
  # System utilities
  - curl
  - wget
  - htop
  
  # Development tools
  - git
  - vim
  - build-essential
  
  # Web server
  - nginx
  - postgresql
  - redis
```

### 4. Verify Critical Components

```yaml
verification:
  required_packages:
    - nginx
    - postgresql
  
  required_services:
    - nginx
    - postgresql
  
  required_commands:
    - name: "Check Nginx config"
      command: "nginx -t"
```

---

**See [GUIDANCE.md](GUIDANCE.md) for usage examples and patterns** 🦀✨

