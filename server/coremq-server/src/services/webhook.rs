use std::sync::{Arc, RwLock};
use std::time::Duration;

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::models::webhook::{events, Webhook, WebhookEvent};
use crate::storage::redb::Storage;

type HmacSha256 = Hmac<Sha256>;

/// Fires HTTP POSTs to registered webhook URLs when broker events occur.
///
/// Holds an in-memory cache of webhooks (refreshed via [`reload`](Self::reload)
/// whenever the API mutates them) so the hot publish path never touches redb.
#[derive(Clone)]
pub struct WebhookDispatcher {
    client: reqwest::Client,
    storage: Arc<Storage>,
    cache: Arc<RwLock<Vec<Webhook>>>,
}

impl WebhookDispatcher {
    pub fn new(storage: Arc<Storage>) -> Arc<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("CoreMQ-Webhook/1.0")
            .build()
            .expect("failed to build webhook HTTP client");

        let dispatcher = Arc::new(Self {
            client,
            storage,
            cache: Arc::new(RwLock::new(Vec::new())),
        });
        dispatcher.reload();
        dispatcher
    }

    /// Refresh the in-memory cache from storage. Call after any create/update/delete.
    pub fn reload(&self) {
        match self.storage.webhook.get_all() {
            Ok(all) => {
                if let Ok(mut guard) = self.cache.write() {
                    *guard = all;
                }
            }
            Err(e) => eprintln!("webhook cache reload failed: {e}"),
        }
    }

    /// Cheap check used to avoid building event payloads on the hot path.
    pub fn any_enabled_for(&self, event: &str) -> bool {
        self.cache
            .read()
            .map(|g| g.iter().any(|w| w.enabled && w.events.iter().any(|e| e == event)))
            .unwrap_or(false)
    }

    /// Fire-and-forget: deliver `event` to every enabled webhook subscribed to it.
    pub fn dispatch(&self, event: WebhookEvent) {
        let hooks: Vec<Webhook> = match self.cache.read() {
            Ok(guard) => guard
                .iter()
                .filter(|w| w.enabled && w.events.iter().any(|e| e == &event.event))
                .filter(|w| topic_matches(w, &event))
                .cloned()
                .collect(),
            Err(_) => return,
        };

        for hook in hooks {
            let client = self.client.clone();
            let event = event.clone();
            tokio::spawn(async move {
                if let Err(e) = deliver(&client, &hook, &event).await {
                    eprintln!("webhook '{}' -> {} failed: {}", hook.name, hook.url, e);
                }
            });
        }
    }

    /// Deliver a single event synchronously (used by the API "test" endpoint).
    pub async fn deliver_once(&self, hook: &Webhook, event: &WebhookEvent) -> Result<u16, String> {
        deliver(&self.client, hook, event).await
    }
}

/// Whether a webhook's topic filter matches this event (only constrains publishes).
fn topic_matches(hook: &Webhook, event: &WebhookEvent) -> bool {
    if event.event != events::MESSAGE_PUBLISHED {
        return true;
    }
    match (&hook.topic_filter, &event.topic) {
        (Some(filter), Some(topic)) if !filter.is_empty() => mqtt_topic_match(filter, topic),
        _ => true,
    }
}

/// Standard MQTT topic-filter matching with `+` (single level) and `#` (multi level).
fn mqtt_topic_match(filter: &str, topic: &str) -> bool {
    let f: Vec<&str> = filter.split('/').collect();
    let t: Vec<&str> = topic.split('/').collect();

    for (i, seg) in f.iter().enumerate() {
        match *seg {
            "#" => return true,
            "+" => {
                if i >= t.len() {
                    return false;
                }
            }
            literal => {
                if i >= t.len() || t[i] != literal {
                    return false;
                }
            }
        }
    }
    f.len() == t.len()
}

async fn deliver(
    client: &reqwest::Client,
    hook: &Webhook,
    event: &WebhookEvent,
) -> Result<u16, String> {
    let body = serde_json::to_vec(event).map_err(|e| e.to_string())?;

    let mut req = client
        .post(&hook.url)
        .header("Content-Type", "application/json")
        .header("X-CoreMQ-Event", event.event.clone());

    for header in &hook.headers {
        if !header.key.trim().is_empty() {
            req = req.header(header.key.as_str(), header.value.as_str());
        }
    }

    if let Some(secret) = &hook.secret {
        if !secret.is_empty() {
            req = req.header("X-CoreMQ-Signature", sign(secret, &body));
        }
    }

    let resp = req.body(body).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if status.is_success() {
        Ok(status.as_u16())
    } else {
        Err(format!("HTTP {}", status.as_u16()))
    }
}

fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}
