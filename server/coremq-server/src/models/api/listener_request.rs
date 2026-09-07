use serde::Deserialize;

use crate::enums::protocol::ProtocolType;

/// Payload to create a new listener at runtime.
#[derive(Debug, Deserialize)]
pub struct CreateListener {
    pub name: String,
    pub protocol: ProtocolType,
    #[serde(default)]
    pub host: Option<String>,
    pub port: u16,
    #[serde(default)]
    pub tls: Option<TlsInput>,
    /// Maximum simultaneous connections (None/0 = unlimited).
    #[serde(default)]
    pub max_connections: Option<u32>,
}

/// TLS material for a tls/wss listener. Each field may be inline PEM content
/// (starting with `-----BEGIN`) or a path to an existing file on the server.
#[derive(Debug, Deserialize)]
pub struct TlsInput {
    pub cert: String,
    pub key: String,
    #[serde(default)]
    pub ca: Option<String>,
}
