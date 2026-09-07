use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

use crate::models::auth::AuthConfig;
use crate::storage::redb::Storage;
use crate::utils;

/// Authenticates MQTT clients on CONNECT using an EMQX-style chain:
/// built-in DB → HTTP(S) → JWT, falling back to `allow_anonymous`.
#[derive(Clone)]
pub struct AuthService {
    storage: Arc<Storage>,
    client: reqwest::Client,
    cache: Arc<RwLock<AuthConfig>>,
}

impl AuthService {
    pub fn new(storage: Arc<Storage>) -> Arc<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build auth HTTP client");

        let svc = Arc::new(Self {
            storage,
            client,
            cache: Arc::new(RwLock::new(AuthConfig::default())),
        });
        svc.reload();
        svc
    }

    /// Refresh the cached config from storage. Call after the admin API updates it.
    pub fn reload(&self) {
        if let Ok(cfg) = self.storage.auth.get_config() {
            if let Ok(mut guard) = self.cache.write() {
                *guard = cfg;
            }
        }
    }

    /// Returns true if the client is allowed to connect.
    pub async fn authenticate(
        &self,
        client_id: &str,
        username: Option<&str>,
        password: Option<&str>,
        peer: SocketAddr,
    ) -> bool {
        let cfg = match self.cache.read() {
            Ok(g) => g.clone(),
            Err(_) => return false,
        };

        // 1) Built-in credential database.
        if cfg.builtin_enabled {
            if let Some(user) = username {
                match self.storage.auth.cred_get(user) {
                    Ok(Some(cred)) => {
                        // Known user: password must match (decisive).
                        return password
                            .map(|p| utils::password::verify(p, &cred.password_hash))
                            .unwrap_or(false);
                    }
                    _ => { /* unknown user → ignore, try next authenticator */ }
                }
            }
        }

        // 2) External HTTP(S) endpoint.
        if cfg.http_enabled && !cfg.http_url.is_empty() {
            match self.http_check(&cfg.http_url, client_id, username, password, peer).await {
                Some(decision) => return decision,
                None => { /* network/other error → ignore */ }
            }
        }

        // 3) JWT: the password is a signed token.
        if cfg.jwt_enabled && !cfg.jwt_secret.is_empty() {
            return match password {
                Some(token) => verify_jwt(token, &cfg.jwt_secret),
                None => false,
            };
        }

        cfg.allow_anonymous
    }

    async fn http_check(
        &self,
        url: &str,
        client_id: &str,
        username: Option<&str>,
        password: Option<&str>,
        peer: SocketAddr,
    ) -> Option<bool> {
        let body = serde_json::json!({
            "clientid": client_id,
            "username": username,
            "password": password,
            "peerhost": peer.ip().to_string(),
        });

        match self.client.post(url).json(&body).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    Some(true)
                } else if status.is_client_error() {
                    Some(false)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}

fn verify_jwt(token: &str, secret: &str) -> bool {
    let validation = Validation::new(Algorithm::HS256);
    decode::<serde_json::Value>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation).is_ok()
}
