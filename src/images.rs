// SPDX-License-Identifier: AGPL-3.0-or-later
//! Image management and discovery

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Image types available
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageType {
    /// Cloud-optimized base images
    Cloud,
    /// ISO installation images
    Iso,
    /// Intermediate build images
    Intermediate,
    /// Final templates
    Template,
    /// Base images
    Base,
}

/// Discovered image
#[derive(Debug, Clone)]
pub struct Image {
    /// File stem or display name.
    pub name: String,
    /// Absolute or workspace-relative path to the image file.
    pub path: PathBuf,
    /// How this file is classified (cloud, ISO, template, etc.).
    pub image_type: ImageType,
    /// File size in bytes.
    pub size_bytes: u64,
}

/// Image manager for reagents directory
pub struct ImageManager {
    reagents_root: PathBuf,
}

impl ImageManager {
    /// Create a new image manager
    pub fn new(reagents_root: impl AsRef<Path>) -> Self {
        Self {
            reagents_root: reagents_root.as_ref().to_path_buf(),
        }
    }

    /// List all cloud images
    pub async fn list_cloud_images(&self) -> Result<Vec<Image>> {
        self.list_images_in_dir(self.reagents_root.join("images/cloud"), ImageType::Cloud)
            .await
    }

    /// List all ISO images
    pub async fn list_iso_images(&self) -> Result<Vec<Image>> {
        self.list_images_in_dir(self.reagents_root.join("isos"), ImageType::Iso)
            .await
    }

    /// List all templates
    pub async fn list_templates(&self) -> Result<Vec<Image>> {
        self.list_images_in_dir(
            self.reagents_root.join("images/templates"),
            ImageType::Template,
        )
        .await
    }

    /// Find a specific cloud image by name
    pub async fn find_cloud_image(&self, name: &str) -> Result<Option<Image>> {
        let images = self.list_cloud_images().await?;
        Ok(images.into_iter().find(|img| img.name.contains(name)))
    }

    /// List images in a directory
    async fn list_images_in_dir(&self, dir: PathBuf, image_type: ImageType) -> Result<Vec<Image>> {
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut images = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir)
            .await
            .context(format!("Failed to read directory: {}", dir.display()))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file()
                && let Some(extension) = path.extension()
            {
                let ext = extension.to_string_lossy();
                if ext == "img" || ext == "qcow2" || ext == "iso" {
                    let metadata = tokio::fs::metadata(&path).await?;
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    images.push(Image {
                        name,
                        path,
                        image_type: image_type.clone(),
                        size_bytes: metadata.len(),
                    });
                }
            }
        }

        Ok(images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_image_type() {
        assert_eq!(ImageType::Cloud, ImageType::Cloud);
        assert_ne!(ImageType::Cloud, ImageType::Iso);
    }

    #[tokio::test]
    async fn list_cloud_images_empty_when_dir_missing() {
        let tmp = TempDir::new().expect("tmpdir");
        let mgr = ImageManager::new(tmp.path());
        let imgs = mgr.list_cloud_images().await.expect("list");
        assert!(imgs.is_empty());
    }

    #[tokio::test]
    async fn list_cloud_images_finds_img_qcow2_iso() {
        let tmp = TempDir::new().expect("tmpdir");
        let cloud = tmp.path().join("images/cloud");
        tokio::fs::create_dir_all(&cloud).await.expect("mkdir");
        tokio::fs::write(cloud.join("base.img"), b"x")
            .await
            .expect("write");
        tokio::fs::write(cloud.join("other.qcow2"), b"yy")
            .await
            .expect("write");
        tokio::fs::write(cloud.join("live.iso"), b"z")
            .await
            .expect("write");
        tokio::fs::write(cloud.join("readme.txt"), b"nope")
            .await
            .expect("write");

        let mgr = ImageManager::new(tmp.path());
        let mut imgs = mgr.list_cloud_images().await.expect("list");
        imgs.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(imgs.len(), 3);
        assert_eq!(imgs[0].name, "base.img");
        assert_eq!(imgs[0].image_type, ImageType::Cloud);
        assert_eq!(imgs[0].size_bytes, 1);
        assert_eq!(imgs[1].name, "live.iso");
        assert_eq!(imgs[2].name, "other.qcow2");
    }

    #[tokio::test]
    async fn list_iso_and_templates_and_find() {
        let tmp = TempDir::new().expect("tmpdir");
        let isos = tmp.path().join("isos");
        let templates = tmp.path().join("images/templates");
        tokio::fs::create_dir_all(&isos).await.expect("mkdir");
        tokio::fs::create_dir_all(&templates).await.expect("mkdir");
        tokio::fs::write(isos.join("install.iso"), b"i")
            .await
            .expect("write");
        tokio::fs::write(templates.join("tmpl.qcow2"), b"q")
            .await
            .expect("write");

        let mgr = ImageManager::new(tmp.path());
        let iso = mgr.list_iso_images().await.expect("isos");
        assert_eq!(iso.len(), 1);
        assert_eq!(iso[0].image_type, ImageType::Iso);

        let t = mgr.list_templates().await.expect("tmpl");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].image_type, ImageType::Template);

        let found = mgr
            .find_cloud_image("missing")
            .await
            .expect("find")
            .is_none();
        assert!(found);
    }
}
