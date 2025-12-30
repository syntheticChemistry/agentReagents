//! Image management and discovery

use anyhow::{Result, Context};
use std::path::{Path, PathBuf};

/// Image types available
#[derive(Debug, Clone, PartialEq)]
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
    pub name: String,
    pub path: PathBuf,
    pub image_type: ImageType,
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
        self.list_images_in_dir(self.reagents_root.join("images/cloud"), ImageType::Cloud).await
    }

    /// List all ISO images
    pub async fn list_iso_images(&self) -> Result<Vec<Image>> {
        self.list_images_in_dir(self.reagents_root.join("isos"), ImageType::Iso).await
    }

    /// List all templates
    pub async fn list_templates(&self) -> Result<Vec<Image>> {
        self.list_images_in_dir(self.reagents_root.join("images/templates"), ImageType::Template).await
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
        let mut entries = tokio::fs::read_dir(&dir).await
            .context(format!("Failed to read directory: {:?}", dir))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    let ext = extension.to_string_lossy();
                    if ext == "img" || ext == "qcow2" || ext == "iso" {
                        let metadata = tokio::fs::metadata(&path).await?;
                        let name = path.file_name()
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
        }

        Ok(images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_type() {
        assert_eq!(ImageType::Cloud, ImageType::Cloud);
        assert_ne!(ImageType::Cloud, ImageType::Iso);
    }
}

