pub mod listener;
pub mod cluster;

use serde::{Deserialize, Serialize};

use crate::models::config::cluster::{ClusterConfig, FederationConfig};
use crate::models::listener::ListenerConfig;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub middleware: Middleware,
    pub mqtt: MqttConfig,

    /*
      Absent means clustering is off. A single-node deployment must not pay for
      any of it, so this stays None rather than defaulting to a disabled struct.
    */
    #[serde(default)]
    pub cluster: Option<ClusterConfig>,

    #[serde(default)]
    pub federation: Vec<FederationConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Middleware {
    pub model_path: String,
    pub policy_path: String,
    pub secret: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MqttConfig {
    pub listeners: Vec<ListenerConfig>,
}

