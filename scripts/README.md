# Scripts

Active automation lives in this directory. **Legacy** Pop!_OS / Cosmic one-off builders and installers are under `legacy/`; see `legacy/DEPRECATED_README.md`.

## Active scripts

| Script | Purpose |
|--------|---------|
| `download-common.sh` | Quick download helper for common resources (sources `configs/defaults.env`) |
| `download-cloud-images.sh` | Fetch cloud base images into the repo layout |
| `download-isos.sh` | Download ISO artifacts |
| `download-packages.sh` | Download `.deb` / package payloads |
| `download-pop-os-24.sh` | Pop!_OS 24–specific artifact download |
| `export-substrates.sh` | Export ionChannel VM substrates for backup or transfer |
| `gather-ingredients.sh` | Mise en place: populate `packages/` for airgap-friendly builds |
| `import-substrates.sh` | Import ionChannel substrates onto a new host (`libvirt` images path) |
| `lint.sh` | Syntax-check active and legacy shell scripts (`bash -n`) |
| `serve-ingredients.sh` | HTTP serve gathered packages for VMs (e.g. libvirt default bridge) |
| `setup-reagents.sh` | Tower setup: download binaries and prepare template resources |
| `update-checksums.sh` | Regenerate `docs/CHECKSUMS.md` from downloaded artifacts |
| `verify-setup.sh` | Verify directory layout, artifacts, and checksums |
