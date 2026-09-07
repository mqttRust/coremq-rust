use serde::{Deserialize, Serialize};

use crate::{enums::protocol::ProtocolType};




#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ListenerConfig {
    pub name: String,
    pub protocol: ProtocolType,
    pub host: String,
    pub port: u16,

    #[serde(default)]
    pub tls: Option<TlsConfig>,

    /// Maximum simultaneous client connections on this listener (None = unlimited).
    #[serde(default)]
    pub max_connections: Option<u32>,
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TlsConfig {
    pub cert: String,
    pub key: String,

    #[serde(default)]
    pub ca: Option<String>,
}


#[derive(Clone, Serialize, Deserialize)]
pub struct  StopListener {
    pub port: u16
}