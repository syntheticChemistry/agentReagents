# agentReagents - Git Repository Setup Guide

This guide shows how to convert the existing agentReagents directory into a git repository for easy cloning to other towers.

## Initial Setup (One-Time on This Tower)

### 1. Initialize Git Repository

```bash
cd /home/nestgate/Development/syntheticChemistry/agentReagents

# Initialize git
git init

# Add all tracked files (scripts, docs, configs)
# Large binaries are excluded via .gitignore
git add .

# Initial commit
git commit -m "Initial commit: agentReagents with automated download scripts

- Scripts for downloading ISOs, cloud images, and packages
- Documentation for setup and usage
- Configs for template building
- .gitignore excludes large binaries (ISOs, images, debs)
- Total repo size: <1MB (excluding binaries)"
```

### 2. Create GitHub Repository

```bash
# Option A: Using GitHub CLI (gh)
gh repo create syntheticChemistry/agentReagents --public --source=. --push

# Option B: Manual (create repo on GitHub first, then:)
git remote add origin https://github.com/syntheticChemistry/agentReagents.git
git branch -M main
git push -u origin main
```

### 3. Verify Repository

```bash
# Check what's tracked
git ls-files | head -20

# Check repo size (should be <1MB)
du -sh .git

# Check what's ignored
git status --ignored
```

## Using on Other Towers

### Clone and Setup

```bash
# Clone the repository
cd ~/Development
git clone https://github.com/syntheticChemistry/agentReagents.git
cd agentReagents

# Run automated setup (downloads all binaries)
bash scripts/setup-reagents.sh

# Verify everything downloaded correctly
bash scripts/verify-setup.sh
```

### What Gets Downloaded

The setup script automatically downloads:
- **ISOs** (~13GB): Pop!_OS 22/24, Ubuntu 24
- **Cloud Images** (~2GB): Ubuntu 22/24 server images
- **Packages** (~18MB): RustDesk and other tools

Total download: ~15GB, takes 15-30 minutes depending on connection.

## Repository Structure

### Tracked in Git (<1MB)
```
scripts/               # Download and build automation
configs/               # Configuration files
docs/                  # Documentation
*.md                   # All markdown documentation
.gitignore            # Excludes large binaries
```

### Downloaded (Not in Git, ~15GB)
```
isos/*.iso            # OS installation ISOs
images/**/*.qcow2     # VM templates
images/**/*.img       # Cloud images
debs/**/*.deb         # Package files
```

## Updating the Repository

### Adding New Scripts/Docs

```bash
cd agentReagents

# Make changes to scripts/docs
vim scripts/new-script.sh

# Commit changes
git add scripts/new-script.sh
git commit -m "Add new build script for XYZ"
git push
```

### Pulling Updates on Other Towers

```bash
cd agentReagents

# Pull latest scripts/docs
git pull

# Re-run setup if needed (only downloads missing files)
bash scripts/setup-reagents.sh

# Verify
bash scripts/verify-setup.sh
```

## For AI Agents

### Clone and Setup Command
```bash
git clone https://github.com/syntheticChemistry/agentReagents.git ~/agentReagents && \
cd ~/agentReagents && \
bash scripts/setup-reagents.sh && \
bash scripts/verify-setup.sh
```

### Agent-Friendly Features

1. **Self-Contained**: Single command setup
2. **Reproducible**: Same scripts work everywhere
3. **Verifiable**: Built-in verification
4. **Documented**: Clear README and SETUP.md
5. **Small Repo**: <1MB git history, fast clone

## Troubleshooting

### Repository Too Large Warning

If you accidentally committed large files:

```bash
# Remove large file from git history
git filter-branch --tree-filter 'rm -f isos/*.iso' HEAD
git push --force

# Or use BFG Repo Cleaner
```

### Missing Files After Clone

```bash
# Run setup script to download everything
bash scripts/setup-reagents.sh
```

### Permission Issues

```bash
# Make scripts executable
chmod +x scripts/*.sh
```

## Benefits of Git Approach vs USB

| Aspect | USB | Git |
|--------|-----|-----|
| **Transfer Size** | 15-25GB | <1MB |
| **Transfer Time** | 10-30 min | <1 min |
| **Updates** | Full recopy | `git pull` |
| **Versioning** | Manual | Automatic |
| **Multi-Tower** | Serial copy | Parallel clone |
| **Agent-Friendly** | Manual | One command |
| **Documentation** | Separate | Integrated |

## Integration with ionChannel

After setting up agentReagents, use it with ionChannel:

```bash
# Clone ionChannel
cd ~/Development/syntheticChemistry
git clone <ionChannel-repo>

# agentReagents is automatically discovered at ../agentReagents
cd ionChannel
cargo run --bin ab-validation --features benchscale
```

## Next Steps

1. **Initialize git** (see step 1 above)
2. **Create GitHub repo** (see step 2 above)
3. **Test on another machine** (clone and run setup)
4. **Document repo URL** in ionChannel README

---

**Ready to initialize?** Run the commands in "Initial Setup" section above! 🚀

