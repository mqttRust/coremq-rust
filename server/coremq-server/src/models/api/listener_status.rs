use serde::Serialize;

use crate::models::listener::ListenerConfig;

/// A listener's configuration plus its live connection count, for the admin API.
#[derive(Debug, Clone, Serialize)]
pub struct ListenerStatus {
    #[serde(flatten)]
    pub config: ListenerConfig,
    /// Number of MQTT clients currently connected on this listener's port.
    pub connections: usize,
}
