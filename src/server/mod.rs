// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON-RPC 2.0 server for agentReagents (UniBin compliance).
//!
//! Implements newline-delimited JSON-RPC over TCP per
//! `PRIMAL_IPC_PROTOCOL.md` v3.1. Exposes `health.*`, `image.*`,
//! `registry.*`, and `template.*` method families.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, warn};

use crate::templates::{TemplateManifest, TemplateRegistry};

/// Settings for capability-based registration with a Unix-socket service registry.
///
/// The server does **not** embed a specific broker or product name. Callers supply the logical
/// [`service_name`](Self::service_name) and the socket path (via [`Self::with_default_socket`]
/// and `REGISTRY_SOCKET`). Deployment manifests or the CLI own defaults such as `"agent-reagents"`.
///
/// **Capability-based pattern:** registration is driven by *where* to connect (`registry_socket`)
/// and *what* is being offered (`service_name`), not by hardcoding a particular ecosystem daemon.
#[derive(Debug, Clone)]
pub struct RegistrationSettings {
    /// Unix domain socket path for the registry (e.g. from `REGISTRY_SOCKET`).
    pub registry_socket: PathBuf,
    /// Logical name this instance registers under (from caller, systemd, or orchestration).
    pub service_name: String,
}

impl RegistrationSettings {
    /// Creates settings with an explicit socket path and service name from the caller.
    #[must_use]
    pub fn new(registry_socket: PathBuf, service_name: String) -> Self {
        Self {
            registry_socket,
            service_name,
        }
    }

    /// Uses `REGISTRY_SOCKET` if set, otherwise `/run/ecoPrimals/registry.sock`.
    /// `service_name` must be supplied by the caller (CLI, config, or tests).
    #[must_use]
    pub fn with_default_socket(service_name: String) -> Self {
        let registry_socket = std::env::var("REGISTRY_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/run/ecoPrimals/registry.sock"));
        Self {
            registry_socket,
            service_name,
        }
    }
}

/// Standard JSON-RPC 2.0 error codes.
mod error_codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;
}

/// JSON-RPC 2.0 request envelope.
#[derive(Debug, Deserialize)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    id: serde_json::Value,
}

/// JSON-RPC 2.0 success response.
#[derive(Debug, Serialize)]
struct RpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
    id: serde_json::Value,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Serialize)]
struct RpcError {
    code: i64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl RpcResponse {
    fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            result: Some(result),
            error: None,
            id,
        }
    }

    fn error(id: serde_json::Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }
}

/// Method dispatch error kinds.
#[derive(Debug)]
enum MethodError {
    /// Method name not recognised.
    NotFound,
    /// Parameters failed validation.
    InvalidParams(String),
    /// Server-side failure.
    Internal(String),
}

/// Shared server state.
struct ServerState {
    registry_dir: PathBuf,
}

/// Run the agentReagents JSON-RPC server.
///
/// Binds TCP on `addr`, accepts newline-delimited JSON-RPC 2.0.
/// `registry_dir` is where templates are stored.
/// `registration` supplies the registry socket path and service name when the process is not in
/// standalone mode; see [`RegistrationSettings`].
pub async fn run_server(
    addr: SocketAddr,
    registry_dir: PathBuf,
    standalone: bool,
    registration: RegistrationSettings,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("agentReagents JSON-RPC server listening on {addr}");

    if standalone {
        info!("running in standalone mode (no registry registration)");
    } else if let Ok(family_id) = std::env::var("FAMILY_ID") {
        info!(
            "FAMILY_ID={family_id} — registry registration not yet implemented (socket={}, service={})",
            registration.registry_socket.display(),
            registration.service_name,
        );
    } else {
        warn!(
            "FAMILY_ID not set and not standalone — degrading to standalone mode (would use socket={}, service={})",
            registration.registry_socket.display(),
            registration.service_name,
        );
    }

    let state = Arc::new(ServerState { registry_dir });

    loop {
        let (stream, peer) = listener.accept().await?;
        info!("accepted connection from {peer}");
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, &state).await {
                error!("connection error from {peer}: {e}");
            }
        });
    }
}

async fn handle_connection(stream: TcpStream, state: &ServerState) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let response = dispatch_request(&line, state);

        let mut out = serde_json::to_vec(&response)?;
        out.push(b'\n');
        writer.write_all(&out).await?;
    }

    Ok(())
}

fn dispatch_request(line: &str, state: &ServerState) -> RpcResponse {
    let request: RpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return RpcResponse::error(
                serde_json::Value::Null,
                error_codes::PARSE_ERROR,
                format!("Parse error: {e}"),
            );
        }
    };

    if request.jsonrpc != "2.0" {
        return RpcResponse::error(
            request.id,
            error_codes::INVALID_REQUEST,
            "Invalid JSON-RPC version (must be \"2.0\")",
        );
    }

    let id = request.id.clone();

    match dispatch_method(&request.method, &request.params, state) {
        Ok(result) => RpcResponse::success(id, result),
        Err(MethodError::NotFound) => RpcResponse::error(
            id,
            error_codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", request.method),
        ),
        Err(MethodError::InvalidParams(msg)) => {
            RpcResponse::error(id, error_codes::INVALID_PARAMS, msg)
        }
        Err(MethodError::Internal(msg)) => RpcResponse::error(id, error_codes::INTERNAL_ERROR, msg),
    }
}

type MethodResult = Result<serde_json::Value, MethodError>;

fn dispatch_method(method: &str, params: &serde_json::Value, state: &ServerState) -> MethodResult {
    match method {
        "health.liveness" => health_liveness(),
        "health.readiness" => health_readiness(state),
        "health.check" => health_check(state),

        "registry.list" => registry_list(state),

        "template.validate" => template_validate(params),

        "image.list" => image_list(state),

        _ => Err(MethodError::NotFound),
    }
}

// ---------------------------------------------------------------------------
// health.*
// ---------------------------------------------------------------------------

#[expect(clippy::unnecessary_wraps, reason = "Uniform MethodResult for JSON-RPC handler table")]
fn health_liveness() -> MethodResult {
    Ok(serde_json::json!({
        "status": "alive",
        "service": "agent-reagents",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

#[expect(clippy::unnecessary_wraps, reason = "Uniform MethodResult for JSON-RPC handler table")]
fn health_readiness(state: &ServerState) -> MethodResult {
    let registry_exists = state.registry_dir.exists();
    Ok(serde_json::json!({
        "status": if registry_exists { "ready" } else { "not_ready" },
        "registry_dir": state.registry_dir.display().to_string(),
    }))
}

#[expect(clippy::unnecessary_wraps, reason = "Uniform MethodResult for JSON-RPC handler table")]
fn health_check(state: &ServerState) -> MethodResult {
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

fn registry_list(state: &ServerState) -> MethodResult {
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

fn template_validate(params: &serde_json::Value) -> MethodResult {
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

fn image_list(state: &ServerState) -> MethodResult {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_response_success() {
        let resp = RpcResponse::success(
            serde_json::Value::Number(1.into()),
            serde_json::json!({"status": "ok"}),
        );
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_rpc_response_error() {
        let resp = RpcResponse::error(
            serde_json::Value::Number(1.into()),
            error_codes::METHOD_NOT_FOUND,
            "not found",
        );
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"error\""));
        assert!(json.contains("-32601"));
    }

    #[test]
    fn test_health_liveness() {
        let result = health_liveness().expect("liveness");
        assert_eq!(result["status"], "alive");
        assert_eq!(result["service"], "agent-reagents");
    }

    #[test]
    fn test_dispatch_unknown_method() {
        let state = ServerState {
            registry_dir: PathBuf::from("/nonexistent"),
        };
        let result = dispatch_method("unknown.method", &serde_json::json!({}), &state);
        assert!(matches!(result, Err(MethodError::NotFound)));
    }

    #[test]
    fn test_dispatch_health_readiness() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let state = ServerState {
            registry_dir: tmp.path().to_path_buf(),
        };
        let result = dispatch_method("health.readiness", &serde_json::json!({}), &state);
        let val = result.expect("readiness");
        assert_eq!(val["status"], "ready");
    }

    #[test]
    fn test_template_validate_missing_path() {
        let result = template_validate(&serde_json::json!({}));
        assert!(matches!(result, Err(MethodError::InvalidParams(_))));
    }

    #[test]
    fn test_full_dispatch_request() {
        let state = ServerState {
            registry_dir: PathBuf::from("/nonexistent"),
        };
        let line = r#"{"jsonrpc":"2.0","method":"health.liveness","params":{},"id":1}"#;
        let resp = dispatch_request(line, &state);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_dispatch_parse_error() {
        let state = ServerState {
            registry_dir: PathBuf::from("/nonexistent"),
        };
        let resp = dispatch_request("not json", &state);
        assert!(resp.error.is_some());
        assert_eq!(
            resp.error.as_ref().expect("err").code,
            error_codes::PARSE_ERROR
        );
    }

    #[test]
    fn test_dispatch_invalid_jsonrpc_version() {
        let state = ServerState {
            registry_dir: PathBuf::from("/nonexistent"),
        };
        let line = r#"{"jsonrpc":"1.0","method":"health.liveness","params":{},"id":1}"#;
        let resp = dispatch_request(line, &state);
        assert!(resp.error.is_some());
        assert_eq!(
            resp.error.as_ref().expect("err").code,
            error_codes::INVALID_REQUEST
        );
    }

    #[test]
    fn template_validate_nonexistent_path_returns_valid_false() {
        let r = template_validate(&serde_json::json!({ "path": "/no/such/file.yaml" }));
        let v = r.expect("ok result");
        assert_eq!(v["valid"], false);
        assert!(v["error"].as_str().unwrap().contains("not found"));
    }

    #[test]
    fn template_validate_valid_manifest_via_temp_file() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let path = tmp.path().join("m.yaml");
        let yaml = r#"
name: demo
version: 1.0.0
base_image: /tmp/x.img
resources:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 30
build_steps: []
verification: {}
"#;
        std::fs::write(&path, yaml).expect("write");

        let r = template_validate(&serde_json::json!({ "path": path.to_str().unwrap() }));
        let v = r.expect("ok");
        assert_eq!(v["valid"], true);
        assert_eq!(v["name"], "demo");
    }

    #[test]
    fn registry_list_and_image_list_with_temp_registry() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let state = ServerState {
            registry_dir: tmp.path().to_path_buf(),
        };
        let reg = dispatch_method("registry.list", &serde_json::json!({}), &state)
            .expect("registry");
        assert_eq!(reg["templates"], serde_json::json!([]));

        let templates_dir = tmp.path().join("templates");
        std::fs::create_dir_all(&templates_dir).expect("mkdir");
        std::fs::write(templates_dir.join("a.qcow2"), b"x").expect("img");

        let imgs = dispatch_method("image.list", &serde_json::json!({}), &state).expect("images");
        assert_eq!(imgs["images"].as_array().expect("arr").len(), 1);
    }

    #[test]
    fn health_check_degraded_when_registry_unusable() {
        let state = ServerState {
            registry_dir: PathBuf::from("/nonexistent/path/that/does/not/exist/reagents"),
        };
        let h = health_check(&state).expect("health");
        assert_eq!(h["status"], "degraded");
    }

    #[test]
    fn health_check_healthy_when_registry_initializes() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let state = ServerState {
            registry_dir: tmp.path().to_path_buf(),
        };
        let h = health_check(&state).expect("health");
        assert_eq!(h["status"], "healthy");
        assert_eq!(h["registry"], true);
        assert_eq!(h["templates"], 0);
    }

    #[test]
    fn readiness_not_ready_when_registry_dir_missing() {
        let state = ServerState {
            registry_dir: PathBuf::from("/this/path/should/not/exist/agent-reagents-readiness"),
        };
        let r = health_readiness(&state).expect("readiness");
        assert_eq!(r["status"], "not_ready");
    }

    #[test]
    fn dispatch_request_method_not_found_envelope() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let state = ServerState {
            registry_dir: tmp.path().to_path_buf(),
        };
        let line = r#"{"jsonrpc":"2.0","method":"no.such.method","params":{},"id":99}"#;
        let resp = dispatch_request(line, &state);
        let err = resp.error.expect("error");
        assert_eq!(err.code, error_codes::METHOD_NOT_FOUND);
        assert!(err.message.contains("no.such.method"));
    }

    #[test]
    fn template_validate_parse_error_in_file() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let path = tmp.path().join("broken.yaml");
        std::fs::write(&path, "this is not: [[[[ valid yaml").expect("write");
        let r = template_validate(&serde_json::json!({ "path": path.to_str().unwrap() }))
            .expect("result");
        assert_eq!(r["valid"], false);
        let err = r["error"].as_str().expect("err str");
        assert!(
            err.contains("parse") || err.contains("YAML") || err.contains("yaml"),
            "{err}"
        );
    }

    #[test]
    fn template_validate_manifest_validation_failure() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let path = tmp.path().join("low_mem.yaml");
        let yaml = r#"
name: demo
version: 1.0.0
base_image: /tmp/x.img
resources:
  memory_mb: 256
  vcpus: 1
  disk_gb: 30
build_steps: []
verification: {}
"#;
        std::fs::write(&path, yaml).expect("write");
        let r = template_validate(&serde_json::json!({ "path": path.to_str().unwrap() }))
            .expect("result");
        assert_eq!(r["valid"], false);
        let err = r["error"].as_str().expect("err");
        assert!(err.contains("512") || err.contains("Memory"), "{err}");
    }

    #[tokio::test]
    async fn tcp_server_accepts_line_delimited_jsonrpc() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpListener;

        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let registry = tmp.path().to_path_buf();

        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        drop(listener);

        let reg = registry.clone();
        let server = tokio::spawn(async move {
            run_server(
                addr,
                reg,
                true,
                RegistrationSettings::new(
                    PathBuf::from("/run/ecoPrimals/registry.sock"),
                    "test".into(),
                ),
            )
            .await
        });

        let mut ok = false;
        for _ in 0..80 {
            if tokio::net::TcpStream::connect(addr).await.is_ok() {
                ok = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        assert!(ok, "server did not accept connections on {addr}");

        let mut stream = tokio::net::TcpStream::connect(addr)
            .await
            .expect("connect");
        let req = r#"{"jsonrpc":"2.0","method":"health.liveness","params":{},"id":42}"#;
        stream.write_all(req.as_bytes()).await.expect("write");
        stream.write_all(b"\n").await.expect("newline");
        stream.write_all(b"\n").await.expect("blank line");
        stream
            .write_all(
                br#"{"jsonrpc":"2.0","method":"health.readiness","params":{},"id":43}"#,
            )
            .await
            .expect("write2");
        stream.write_all(b"\n").await.expect("newline2");

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("read1");
        let v: serde_json::Value = serde_json::from_str(line.trim()).expect("json1");
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["id"], 42);
        assert_eq!(v["result"]["status"], "alive");

        line.clear();
        reader.read_line(&mut line).await.expect("read2");
        let v2: serde_json::Value = serde_json::from_str(line.trim()).expect("json2");
        assert_eq!(v2["id"], 43);
        assert_eq!(v2["result"]["status"], "ready");

        server.abort();
    }
}
