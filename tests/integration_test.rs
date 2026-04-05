// SPDX-License-Identifier: AGPL-3.0-or-later
//! Integration tests for agentReagents
//!
//! These tests verify the integration between components

use agent_reagents::templates::{TemplateManifest, TemplateRegistry};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_template_manifest_load() {
    let manifest_path = PathBuf::from("templates/ubuntu-24-04-desktop.yaml");

    if !manifest_path.exists() {
        println!("Skipping test - manifest file not found");
        return;
    }

    let manifest =
        TemplateManifest::from_yaml_file(&manifest_path).expect("Failed to load manifest");

    assert_eq!(manifest.name, "ubuntu-24-04-desktop");
    assert_eq!(manifest.version, "1.0.0");
    assert!(manifest.resources.memory_mb >= 512);
    assert!(manifest.resources.vcpus >= 1);
}

#[test]
fn test_template_manifest_validation() {
    let manifest_path = PathBuf::from("templates/popos-24-cosmic.yaml");

    if !manifest_path.exists() {
        println!("Skipping test - manifest file not found");
        return;
    }

    let manifest =
        TemplateManifest::from_yaml_file(&manifest_path).expect("Failed to load manifest");

    // Should pass validation
    manifest.validate().expect("Manifest should be valid");
}

#[test]
fn test_template_registry_creation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let registry = TemplateRegistry::new(temp_dir.path()).expect("Failed to create registry");

    let templates = registry.list_templates();
    assert_eq!(templates.len(), 0);
}

#[test]
fn test_all_manifests_valid() {
    let manifest_files = [
        "templates/ubuntu-24-04-desktop.yaml",
        "templates/popos-24-cosmic.yaml",
        "templates/ubuntu-24-04-rustdesk.yaml",
    ];

    for manifest_file in &manifest_files {
        let path = PathBuf::from(manifest_file);

        if !path.exists() {
            println!("Skipping {} - file not found", manifest_file);
            continue;
        }

        let manifest = TemplateManifest::from_yaml_file(&path)
            .unwrap_or_else(|e| panic!("Failed to load {}: {}", manifest_file, e));

        manifest
            .validate()
            .unwrap_or_else(|e| panic!("{} validation failed: {}", manifest_file, e));

        println!("✓ {} is valid", manifest_file);
    }
}

#[test]
fn test_manifest_round_trip() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let manifest_path = PathBuf::from("templates/ubuntu-24-04-desktop.yaml");

    if !manifest_path.exists() {
        println!("Skipping test - manifest file not found");
        return;
    }

    // Load manifest
    let manifest =
        TemplateManifest::from_yaml_file(&manifest_path).expect("Failed to load manifest");

    // Save to temp file
    let temp_manifest = temp_dir.path().join("test.yaml");
    manifest
        .to_yaml_file(&temp_manifest)
        .expect("Failed to save manifest");

    // Load again
    let manifest2 =
        TemplateManifest::from_yaml_file(&temp_manifest).expect("Failed to reload manifest");

    // Should match
    assert_eq!(manifest.name, manifest2.name);
    assert_eq!(manifest.version, manifest2.version);
    assert_eq!(manifest.base_image, manifest2.base_image);
}

#[test]
fn test_gate_templates_valid() {
    let gate_templates = [
        "templates/gates/gate-ubuntu24-biomeos.yaml",
        "templates/gates/gate-ubuntu24-gpu-sovereign.yaml",
        "templates/gates/gate-aarch64-pixelgate.yaml",
    ];

    for template_file in &gate_templates {
        let path = PathBuf::from(template_file);
        assert!(path.exists(), "gate template missing: {template_file}");

        let manifest = TemplateManifest::from_yaml_file(&path)
            .unwrap_or_else(|e| panic!("failed to load {template_file}: {e}"));

        manifest
            .validate()
            .unwrap_or_else(|e| panic!("{template_file} validation failed: {e}"));

        assert!(
            manifest.name.starts_with("gate-"),
            "{template_file}: gate templates must be named gate-*"
        );

        assert!(
            manifest.metadata.contains_key("gate_standard"),
            "{template_file}: missing gate_standard metadata"
        );

        assert!(
            manifest.metadata.contains_key("arch"),
            "{template_file}: missing arch metadata"
        );
    }
}
