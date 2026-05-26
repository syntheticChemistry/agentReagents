// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON-RPC method handlers for each method family.
//!
//! Each handler receives the JSON-RPC params and shared [`ServerState`]
//! reference, returning a [`MethodResult`]. New method families are added
//! here; the dispatch table in [`super::dispatch_method`] maps names to
//! these functions.

use super::{MethodError, MethodResult, ServerState};
use crate::templates::{TemplateManifest, TemplateRegistry};

// ---------------------------------------------------------------------------
// health.*
// ---------------------------------------------------------------------------

#[expect(
    clippy::unnecessary_wraps,
    reason = "Uniform MethodResult for JSON-RPC handler table"
)]
pub(super) fn health_liveness() -> MethodResult {
    Ok(serde_json::json!({
        "status": "alive",
        "service": "agent-reagents",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Uniform MethodResult for JSON-RPC handler table"
)]
pub(super) fn health_readiness(state: &ServerState) -> MethodResult {
    let registry_exists = state.registry_dir.exists();
    Ok(serde_json::json!({
        "status": if registry_exists { "ready" } else { "not_ready" },
        "registry_dir": state.registry_dir.display().to_string(),
    }))
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "Uniform MethodResult for JSON-RPC handler table"
)]
pub(super) fn health_check(state: &ServerState) -> MethodResult {
    let registry_ok = TemplateRegistry::new(&state.registry_dir).is_ok();
    let template_count = TemplateRegistry::new(&state.registry_dir)
        .map(|r| r.list_templates().len())
        .unwrap_or(0);

    Ok(serde_json::json!({
        "status": if registry_ok { "healthy" } else { "degraded" },
        "registry": registry_ok,
        "templates": template_count,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ---------------------------------------------------------------------------
// registry.*
// ---------------------------------------------------------------------------

pub(super) fn registry_list(state: &ServerState) -> MethodResult {
    let registry = TemplateRegistry::new(&state.registry_dir)
        .map_err(|e| MethodError::Internal(format!("registry init: {e}")))?;

    let templates: Vec<serde_json::Value> = registry
        .list_templates()
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "version": t.version,
                "size_bytes": t.size_bytes,
                "verified": t.verified,
            })
        })
        .collect();

    Ok(serde_json::json!({ "templates": templates }))
}

// ---------------------------------------------------------------------------
// template.*
// ---------------------------------------------------------------------------

pub(super) fn template_validate(params: &serde_json::Value) -> MethodResult {
    let path_str = params
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| MethodError::InvalidParams("missing \"path\" string".into()))?;

    let path = std::path::Path::new(path_str);
    if !path.exists() {
        return Ok(serde_json::json!({
            "valid": false,
            "error": format!("file not found: {path_str}"),
        }));
    }

    match TemplateManifest::from_yaml_file(path) {
        Ok(manifest) => match manifest.validate() {
            Ok(()) => Ok(serde_json::json!({
                "valid": true,
                "name": manifest.name,
                "version": manifest.version,
                "build_steps": manifest.build_steps.len(),
            })),
            Err(e) => Ok(serde_json::json!({
                "valid": false,
                "error": e.to_string(),
            })),
        },
        Err(e) => Ok(serde_json::json!({
            "valid": false,
            "error": format!("parse error: {e}"),
        })),
    }
}

// ---------------------------------------------------------------------------
// image.*
// ---------------------------------------------------------------------------

pub(super) fn image_list(state: &ServerState) -> MethodResult {
    let images_dir = state.registry_dir.join("templates");
    if !images_dir.exists() {
        return Ok(serde_json::json!({ "images": [] }));
    }

    let images: Vec<serde_json::Value> = std::fs::read_dir(&images_dir)
        .map_err(|e| MethodError::Internal(format!("read dir: {e}")))?
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "qcow2" || ext == "img")
        })
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(serde_json::json!({
                "name": e.file_name().to_string_lossy(),
                "size_bytes": meta.len(),
            }))
        })
        .collect();

    Ok(serde_json::json!({ "images": images }))
}
