use serde::{Deserialize, Serialize};

/// Canonical event names a webhook can subscribe to.
pub mod events {
    pub const CLIENT_CONNECTED: &str = "client.connected";
    pub const CLIENT_DISCONNECTED: &str = "client.disconnected";
    pub const MESSAGE_PUBLISHED: &str = "message.published";
    pub const SUBSCRIPTION_CREATED: &str = "subscription.created";
    pub const SUBSCRIPTION_REMOVED: &str = "subscription.removed";

    pub const ALL: [&str; 5] = [
        CLIENT_CONNECTED,
        CLIENT_DISCONNECTED,
        MESSAGE_PUBLISHED,
        SUBSCRIPTION_CREATED,
        SUBSCRIPTION_REMOVED,
    ];
}

/// A single custom HTTP header sent with every webhook delivery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHeader {
    pub key: String,
    pub value: String,
}

/// A persisted webhook subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Webhook {
    pub id: String,
    pub name: String,
    pub url: String,
    /// Event names this webhook fires for (see [`events`]).
    pub events: Vec<String>,
    /// Optional MQTT-style topic filter; only applies to `message.published`.
    #[serde(default)]
    pub topic_filter: Option<String>,
    /// Custom HTTP headers added to every delivery.
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
    pub enabled: bool,
    /// Optional shared secret; when set, deliveries carry an `X-CoreMQ-Signature`
    /// header = hex(HMAC-SHA256(secret, body)).
    #[serde(default)]
    pub secret: Option<String>,
    pub created_at: String,
}

/// Create/update payload (no server-managed fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookInput {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default)]
    pub topic_filter: Option<String>,
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub secret: Option<String>,
}

fn default_true() -> bool {
    true
}

/// The JSON body delivered to a webhook URL on an event.
#[derive(Debug, Clone, Serialize)]
pub struct WebhookEvent {
    pub event: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_addr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retain: Option<bool>,
}

impl WebhookEvent {
    /// Start a new event of the given type, stamped with the current time.
    pub fn new(event: &str) -> Self {
        Self {
            event: event.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            client_id: None,
            username: None,
            remote_addr: None,
            topic: None,
            payload: None,
            qos: None,
            retain: None,
        }
    }
}
