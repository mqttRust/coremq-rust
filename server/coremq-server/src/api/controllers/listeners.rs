use std::{fs, path::PathBuf};

use axum::{Json, extract::{Path, State}, http::StatusCode};
use tokio::sync::oneshot;

use crate::{
    api::api_state::{ApiResponse, ApiState},
    engine::AdminCommand,
    enums::protocol::ProtocolType,
    models::{
        listener::{ListenerConfig, TlsConfig},
        listener_request::{CreateListener, TlsInput},
        listener_status::ListenerStatus,
    },
};

pub async  fn get_listeners(
    State(state): State<ApiState>
) -> Result<Json<Vec<ListenerStatus>>, StatusCode> {
    let (reply_tx, reply_rx) = oneshot::channel();
    state.engine.send(AdminCommand::GetListeners(reply_tx)).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let listeners = reply_rx.await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(listeners))
}

pub async fn create_listener(
    State(state): State<ApiState>,
    Json(req): Json<CreateListener>,
) -> (StatusCode, Json<ApiResponse<ListenerConfig>>) {
    let cfg = match build_config(req) {
        Ok(cfg) => cfg,
        Err(msg) => return bad(&msg),
    };
    start_and_persist(&state, cfg).await
}

/// PUT /api/v1/listeners/:port — reconfigure a listener: stop the one on `port`,
/// then start it with the new settings (port may change).
pub async fn update_listener(
    Path(port): Path<u16>,
    State(state): State<ApiState>,
    Json(req): Json<CreateListener>,
) -> (StatusCode, Json<ApiResponse<ListenerConfig>>) {
    let cfg = match build_config(req) {
        Ok(cfg) => cfg,
        Err(msg) => return bad(&msg),
    };

    // Stop the existing listener (waits for the socket to be released) and forget it.
    let _ = state.engine.send(AdminCommand::StopListener(port));
    let _ = state.storage.listener.delete(port);

    start_and_persist(&state, cfg).await
}

/// Validate a request and build a `ListenerConfig` (writing any inline TLS PEM to disk).
fn build_config(req: CreateListener) -> Result<ListenerConfig, String> {
    if req.name.trim().is_empty() {
        return Err("name is required".to_string());
    }
    if req.port == 0 {
        return Err("a valid port is required".to_string());
    }

    let needs_tls = matches!(req.protocol, ProtocolType::Tls | ProtocolType::Wss);
    let tls = if needs_tls { Some(build_tls(req.port, req.tls.as_ref())?) } else { None };

    Ok(ListenerConfig {
        name: req.name,
        protocol: req.protocol,
        host: req.host.filter(|h| !h.trim().is_empty()).unwrap_or_else(|| "0.0.0.0".to_string()),
        port: req.port,
        tls,
        max_connections: req.max_connections.filter(|&m| m > 0),
    })
}

/// Ask the engine to start the listener; on success persist it.
async fn start_and_persist(
    state: &ApiState,
    cfg: ListenerConfig,
) -> (StatusCode, Json<ApiResponse<ListenerConfig>>) {
    let (reply_tx, reply_rx) = oneshot::channel();
    if state.engine.send(AdminCommand::StartListener(cfg.clone(), reply_tx)).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, "engine unavailable")));
    }

    match reply_rx.await {
        Ok(Ok(())) => {
            let _ = state.storage.listener.upsert(&cfg);
            (StatusCode::CREATED, Json(ApiResponse::success(cfg, "listener started")))
        }
        Ok(Err(msg)) => bad(&msg),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, "engine did not respond"))),
    }
}

pub async  fn stop_listener(
    Path(port): Path<u16>,
    State(state): State<ApiState>
) -> Result<Json<String>, StatusCode> {
    state.engine.send(AdminCommand::StopListener(port)).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // Forget any persisted (dynamically-created) listener so it doesn't respawn on restart.
    let _ = state.storage.listener.delete(port);
    Ok(Json(String::from("successfully stopped")))
}

fn bad(msg: &str) -> (StatusCode, Json<ApiResponse<ListenerConfig>>) {
    (StatusCode::BAD_REQUEST, Json(ApiResponse::error(StatusCode::BAD_REQUEST, msg)))
}

/// Resolve the TLS input into on-disk cert/key paths (writing inline PEM to the data dir).
fn build_tls(port: u16, input: Option<&TlsInput>) -> Result<TlsConfig, String> {
    let input = input.ok_or_else(|| "cert and key are required for tls/wss listeners".to_string())?;
    let dir = tls_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create tls dir: {e}"))?;

    let cert = resolve_material(&dir, &format!("{port}-cert.pem"), &input.cert, "cert")?;
    let key = resolve_material(&dir, &format!("{port}-key.pem"), &input.key, "key")?;
    let ca = match input.ca.as_ref() {
        Some(ca) if !ca.trim().is_empty() => Some(resolve_material(&dir, &format!("{port}-ca.pem"), ca, "ca")?),
        _ => None,
    };

    Ok(TlsConfig { cert, key, ca })
}

/// If `value` is inline PEM, write it to `dir/filename` and return that path;
/// otherwise treat `value` as an existing file path.
fn resolve_material(dir: &PathBuf, filename: &str, value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{label} is required"));
    }
    if trimmed.starts_with("-----BEGIN") {
        let path = dir.join(filename);
        fs::write(&path, trimmed).map_err(|e| format!("write {label}: {e}"))?;
        Ok(path.to_string_lossy().to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn tls_dir() -> PathBuf {
    let data = std::env::var("COREMQ_DATA").unwrap_or_else(|_| "/etc/coremq/data".to_string());
    PathBuf::from(data).join("tls")
}
