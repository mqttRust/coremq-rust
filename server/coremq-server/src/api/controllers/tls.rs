use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::api::api_state::{ApiResponse, ApiState};

#[derive(Debug, Deserialize)]
pub struct GenerateCert {
    #[serde(default)]
    pub common_name: Option<String>,
    /// Extra subject alternative names (DNS or IP).
    #[serde(default)]
    pub sans: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GeneratedCert {
    /// PEM-encoded self-signed certificate.
    pub cert: String,
    /// PEM-encoded private key.
    pub key: String,
    /// The names embedded in the certificate.
    pub names: Vec<String>,
}

/// POST /api/v1/tls/generate — generate a self-signed certificate + key.
/// The client zips the returned PEM strings for download.
pub async fn generate_cert(
    State(_state): State<ApiState>,
    Json(req): Json<GenerateCert>,
) -> (StatusCode, Json<ApiResponse<GeneratedCert>>) {
    let cn = req
        .common_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "localhost".to_string());

    let mut names: Vec<String> = vec![cn];
    for san in req.sans {
        let san = san.trim().to_string();
        if !san.is_empty() && !names.contains(&san) {
            names.push(san);
        }
    }

    match rcgen::generate_simple_self_signed(names.clone()) {
        Ok(certified) => {
            let generated = GeneratedCert {
                cert: certified.cert.pem(),
                key: certified.key_pair.serialize_pem(),
                names,
            };
            (StatusCode::OK, Json(ApiResponse::success(generated, "certificate generated")))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, format!("failed to generate certificate: {e}"))),
        ),
    }
}
