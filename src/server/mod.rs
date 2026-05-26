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

mod handlers;


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
        let registry_socket = std::env::var("REGISTRY_SOCKET").map_or_else(|_| PathBuf::from("/run/ecoPrimals/registry.sock"), PathBuf::from);
        Self {
            registry_socket,
            service_name,
        }
    }
}

/// Standard JSON-RPC 2.0 error codes.
mod error_codes {
    pub(super) const PARSE_ERROR: i64 = -32700;
    pub(super) const INVALID_REQUEST: i64 = -32600;
    pub(super) const METHOD_NOT_FOUND: i64 = -32601;
    pub(super) const INVALID_PARAMS: i64 = -32602;
    pub(super) const INTERNAL_ERROR: i64 = -32603;
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
        match register_with_registry(&registration, &family_id, addr).await {
            Ok(()) => info!(
                "registered with registry as '{}' (family={family_id})",
                registration.service_name,
            ),
            Err(e) => warn!(
                "registry registration failed (degrading to standalone): {e}"
            ),
        }
    } else {
        warn!(
            "FAMILY_ID not set and not standalone — degrading to standalone mode"
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

/// Register this server with a Unix-socket service registry via JSON-RPC 2.0.
///
/// Sends a `registry.register` call containing the service name, listen address,
/// family ID, and declared capabilities. Non-fatal — the server continues if registration fails.
async fn register_with_registry(
    settings: &RegistrationSettings,
    family_id: &str,
    listen_addr: SocketAddr,
) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let stream = UnixStream::connect(&settings.registry_socket)
        .await
        .map_err(|e| anyhow::anyhow!(
            "cannot connect to registry at {}: {e}",
            settings.registry_socket.display(),
        ))?;

    let (reader, mut writer) = stream.into_split();

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "registry.register",
        "params": {
            "service": settings.service_name,
            "address": listen_addr.to_string(),
            "family_id": family_id,
            "capabilities": [
                "image.build",
                "image.list",
                "template.validate",
                "health.check",
            ],
            "version": env!("CARGO_PKG_VERSION"),
        },
        "id": 1,
    });

    let mut payload = serde_json::to_vec(&request)?;
    payload.push(b'\n');
    writer.write_all(&payload).await?;

    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.read_line(&mut line),
    )
    .await
    .map_err(|_| anyhow::anyhow!("registry response timeout"))??;

    let resp: serde_json::Value = serde_json::from_str(line.trim())?;
    if let Some(err) = resp.get("error") {
        anyhow::bail!("registry rejected registration: {err}");
    }

    info!("registry acknowledged registration: {}", resp);
    Ok(())
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
        "health.liveness" => handlers::health_liveness(),
        "health.readiness" => handlers::health_readiness(state),
        "health.check" => handlers::health_check(state),

        "registry.list" => handlers::registry_list(state),

        "template.validate" => handlers::template_validate(params),

        "image.list" => handlers::image_list(state),

        _ => Err(MethodError::NotFound),
    }
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
        let result = handlers::health_liveness().expect("liveness");
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
        let result = handlers::template_validate(&serde_json::json!({}));
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
        let r = handlers::template_validate(&serde_json::json!({ "path": "/no/such/file.yaml" }));
        let v = r.expect("ok result");
        assert_eq!(v["valid"], false);
        assert!(v["error"].as_str().unwrap().contains("not found"));
    }

    #[test]
    fn template_validate_valid_manifest_via_temp_file() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let path = tmp.path().join("m.yaml");
        let yaml = r"
name: demo
version: 1.0.0
base_image: /tmp/x.img
resources:
  memory_mb: 2048
  vcpus: 2
  disk_gb: 30
build_steps: []
verification: {}
";
        std::fs::write(&path, yaml).expect("write");

        let r = handlers::template_validate(&serde_json::json!({ "path": path.to_str().unwrap() }));
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
        let reg =
            dispatch_method("registry.list", &serde_json::json!({}), &state).expect("registry");
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
        let h = handlers::health_check(&state).expect("health");
        assert_eq!(h["status"], "degraded");
    }

    #[test]
    fn health_check_healthy_when_registry_initializes() {
        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let state = ServerState {
            registry_dir: tmp.path().to_path_buf(),
        };
        let h = handlers::health_check(&state).expect("health");
        assert_eq!(h["status"], "healthy");
        assert_eq!(h["registry"], true);
        assert_eq!(h["templates"], 0);
    }

    #[test]
    fn readiness_not_ready_when_registry_dir_missing() {
        let state = ServerState {
            registry_dir: PathBuf::from("/this/path/should/not/exist/agent-reagents-readiness"),
        };
        let r = handlers::health_readiness(&state).expect("readiness");
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
        let r = handlers::template_validate(&serde_json::json!({ "path": path.to_str().unwrap() }))
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
        let yaml = r"
name: demo
version: 1.0.0
base_image: /tmp/x.img
resources:
  memory_mb: 256
  vcpus: 1
  disk_gb: 30
build_steps: []
verification: {}
";
        std::fs::write(&path, yaml).expect("write");
        let r = handlers::template_validate(&serde_json::json!({ "path": path.to_str().unwrap() }))
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

        let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
        let req = r#"{"jsonrpc":"2.0","method":"health.liveness","params":{},"id":42}"#;
        stream.write_all(req.as_bytes()).await.expect("write");
        stream.write_all(b"\n").await.expect("newline");
        stream.write_all(b"\n").await.expect("blank line");
        stream
            .write_all(br#"{"jsonrpc":"2.0","method":"health.readiness","params":{},"id":43}"#)
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

    #[tokio::test]
    async fn register_with_registry_sends_correct_payload() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixListener;

        let tmp = tempfile::TempDir::new().expect("tmpdir");
        let sock_path = tmp.path().join("registry.sock");

        let listener = UnixListener::bind(&sock_path).expect("bind UDS");

        let settings = RegistrationSettings::new(sock_path, "agent-reagents".into());
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();

        let mock_registry = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader);
            let mut line = String::new();
            lines.read_line(&mut line).await.expect("read");

            let req: serde_json::Value = serde_json::from_str(line.trim()).expect("parse");
            assert_eq!(req["jsonrpc"], "2.0");
            assert_eq!(req["method"], "registry.register");
            assert_eq!(req["params"]["service"], "agent-reagents");
            assert_eq!(req["params"]["address"], "127.0.0.1:9999");
            assert!(req["params"]["capabilities"].as_array().unwrap().len() >= 4);

            let resp = serde_json::json!({"jsonrpc":"2.0","result":{"registered":true},"id":1});
            let mut payload = serde_json::to_vec(&resp).unwrap();
            payload.push(b'\n');
            writer.write_all(&payload).await.expect("write");

            req
        });

        register_with_registry(&settings, "test-family", addr)
            .await
            .expect("registration should succeed");

        let captured_req = mock_registry.await.expect("mock registry task");
        assert_eq!(captured_req["params"]["family_id"], "test-family");
    }

    #[tokio::test]
    async fn register_with_registry_fails_gracefully_on_missing_socket() {
        let settings = RegistrationSettings::new(
            PathBuf::from("/nonexistent/registry.sock"),
            "test".into(),
        );
        let addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        let result = register_with_registry(&settings, "fam", addr).await;
        assert!(result.is_err());
    }
}
