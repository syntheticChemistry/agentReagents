// SPDX-License-Identifier: AGPL-3.0-or-later
//! Package management

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Package type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageType {
    /// Debian package (.deb)
    Deb,
    /// RPM package (.rpm)
    Rpm,
    /// Tarball (.tar.gz, .tar.xz)
    Tarball,
    /// Binary
    Binary,
}

/// Discovered package
#[derive(Debug, Clone)]
pub struct Package {
    /// File name (without guaranteed semantic version parsing).
    pub name: String,
    /// Path to the package file under `reagents/`.
    pub path: PathBuf,
    /// Inferred archive or package kind.
    pub package_type: PackageType,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Package manager for reagents directory
pub struct PackageManager {
    reagents_root: PathBuf,
}

impl PackageManager {
    /// Create a new package manager
    pub fn new(reagents_root: impl AsRef<Path>) -> Self {
        Self {
            reagents_root: reagents_root.as_ref().to_path_buf(),
        }
    }

    /// List all packages
    pub async fn list_packages(&self) -> Result<Vec<Package>> {
        let packages_dir = self.reagents_root.join("packages");

        if !packages_dir.exists() {
            return Ok(vec![]);
        }

        let mut packages = Vec::new();
        self.scan_directory(&packages_dir, &mut packages).await?;

        Ok(packages)
    }

    /// Find a specific package by name
    pub async fn find_package(&self, name: &str) -> Result<Option<Package>> {
        let packages = self.list_packages().await?;
        Ok(packages.into_iter().find(|pkg| pkg.name.contains(name)))
    }

    /// Recursively scan directory for packages (iterative, `Send` for async callers).
    async fn scan_directory(&self, dir: &Path, packages: &mut Vec<Package>) -> Result<()> {
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current) = stack.pop() {
            let mut entries = tokio::fs::read_dir(&current)
                .await
                .context(format!("Failed to read directory: {}", current.display()))?;

            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file()
                    && let Some(package) = self.parse_package(&path).await?
                {
                    packages.push(package);
                }
            }
        }

        Ok(())
    }

    /// Parse a file as a package
    async fn parse_package(&self, path: &Path) -> Result<Option<Package>> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let package_type = match extension {
            "deb" => Some(PackageType::Deb),
            "rpm" => Some(PackageType::Rpm),
            "gz" | "xz" if path.to_string_lossy().contains(".tar.") => Some(PackageType::Tarball),
            _ => None,
        };

        if let Some(pkg_type) = package_type {
            let metadata = tokio::fs::metadata(path).await?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            Ok(Some(Package {
                name,
                path: path.to_path_buf(),
                package_type: pkg_type,
                size_bytes: metadata.len(),
            }))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_package_type() {
        assert_eq!(PackageType::Deb, PackageType::Deb);
        assert_ne!(PackageType::Deb, PackageType::Rpm);
    }

    #[tokio::test]
    async fn list_packages_empty_when_missing() {
        let tmp = TempDir::new().expect("tmpdir");
        let mgr = PackageManager::new(tmp.path());
        assert!(mgr.list_packages().await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn scan_finds_deb_rpm_tarball_nested() {
        let tmp = TempDir::new().expect("tmpdir");
        let root = tmp.path().join("packages");
        let nested = root.join("nest");
        tokio::fs::create_dir_all(&nested).await.expect("mkdir");
        tokio::fs::write(root.join("a.deb"), b"d").await.expect("write");
        tokio::fs::write(nested.join("b.rpm"), b"r").await.expect("write");
        tokio::fs::write(nested.join("c.tar.gz"), b"t").await.expect("write");
        tokio::fs::write(nested.join("note.txt"), b"x").await.expect("write");

        let mgr = PackageManager::new(tmp.path());
        let mut pkgs = mgr.list_packages().await.expect("list");
        pkgs.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(pkgs.len(), 3);
        assert_eq!(pkgs[0].package_type, PackageType::Deb);
        assert_eq!(pkgs[1].package_type, PackageType::Rpm);
        assert_eq!(pkgs[2].package_type, PackageType::Tarball);

        let hit = mgr.find_package("b.rpm").await.expect("find");
        assert!(hit.is_some());
        assert!(mgr.find_package("nope").await.expect("find").is_none());
    }
}
