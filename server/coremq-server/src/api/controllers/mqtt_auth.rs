use axum::{Json, extract::{Path, State}, http::StatusCode};

use crate::{
    api::api_state::{ApiResponse, ApiState},
    models::auth::{AuthConfig, CredentialInput, MqttCredential},
    utils::password::hash_password,
};

/// GET /api/v1/mqtt-auth/config
pub async fn get_config(
    State(state): State<ApiState>,
) -> (StatusCode, Json<ApiResponse<AuthConfig>>) {
    match state.storage.auth.get_config() {
        Ok(cfg) => (StatusCode::OK, Json(ApiResponse::success(cfg, "ok"))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))),
    }
}

/// PUT /api/v1/mqtt-auth/config
pub async fn update_config(
    State(state): State<ApiState>,
    Json(cfg): Json<AuthConfig>,
) -> (StatusCode, Json<ApiResponse<AuthConfig>>) {
    match state.storage.auth.set_config(&cfg) {
        Ok(()) => {
            state.auth.reload();
            (StatusCode::OK, Json(ApiResponse::success(cfg, "authentication settings saved")))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))),
    }
}

/// GET /api/v1/mqtt-auth/credentials — usernames only (never expose hashes).
pub async fn list_credentials(
    State(state): State<ApiState>,
) -> (StatusCode, Json<ApiResponse<Vec<String>>>) {
    match state.storage.auth.cred_all() {
        Ok(creds) => {
            let usernames = creds.into_iter().map(|c| c.username).collect();
            (StatusCode::OK, Json(ApiResponse::success(usernames, "ok")))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))),
    }
}

/// POST /api/v1/mqtt-auth/credentials
pub async fn create_credential(
    State(state): State<ApiState>,
    Json(input): Json<CredentialInput>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    if input.username.trim().is_empty() || input.password.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse::error(StatusCode::BAD_REQUEST, "username and password are required")));
    }

    let password_hash = match hash_password(&input.password) {
        Ok(h) => h,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))),
    };

    let cred = MqttCredential { username: input.username.trim().to_string(), password_hash };
    match state.storage.auth.cred_upsert(&cred) {
        Ok(()) => (StatusCode::CREATED, Json(ApiResponse::success(cred.username, "credential saved"))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))),
    }
}

/// DELETE /api/v1/mqtt-auth/credentials/:username
pub async fn delete_credential(
    State(state): State<ApiState>,
    Path(username): Path<String>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match state.storage.auth.cred_delete(&username) {
        Ok(true) => (StatusCode::OK, Json(ApiResponse::success(username, "credential deleted"))),
        Ok(false) => (StatusCode::NOT_FOUND, Json(ApiResponse::error(StatusCode::NOT_FOUND, "credential not found"))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))),
    }
}
