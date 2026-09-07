use serde::{Deserialize, Serialize};

/// MQTT client authentication settings (EMQX-style chain: builtin → http → jwt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Allow clients that no enabled authenticator decided on (default true = open broker).
    #[serde(default = "default_true")]
    pub allow_anonymous: bool,

    /// Built-in username/password database.
    #[serde(default)]
    pub builtin_enabled: bool,

    /// External HTTP(S) authentication endpoint.
    #[serde(default)]
    pub http_enabled: bool,
    #[serde(default)]
    pub http_url: String,

    /// JWT: the client's password is treated as an HS256 JWT.
    #[serde(default)]
    pub jwt_enabled: bool,
    #[serde(default)]
    pub jwt_secret: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            allow_anonymous: true,
            builtin_enabled: false,
            http_enabled: false,
            http_url: String::new(),
            jwt_enabled: false,
            jwt_secret: String::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

/// A built-in MQTT credential (username + hashed password).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttCredential {
    pub username: String,
    pub password_hash: String,
}

/// Create-credential payload (plaintext password, hashed server-side).
#[derive(Debug, Deserialize)]
pub struct CredentialInput {
    pub username: String,
    pub password: String,
}
