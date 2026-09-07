use axum::{Json, extract::{Path, State}, http::StatusCode};

use crate::{
    api::api_state::{ApiResponse, ApiState},
    models::webhook::{events, Webhook, WebhookEvent, WebhookInput},
};

/// GET /api/v1/webhooks — list all webhooks.
pub async fn list_webhooks(
    State(state): State<ApiState>,
) -> (StatusCode, Json<ApiResponse<Vec<Webhook>>>) {
    match state.storage.webhook.get_all() {
        Ok(hooks) => (StatusCode::OK, Json(ApiResponse::success(hooks, "ok"))),
        Err(e) => internal::<Vec<Webhook>>(e.to_string()),
    }
}

/// POST /api/v1/webhooks — create a webhook.
pub async fn create_webhook(
    State(state): State<ApiState>,
    Json(input): Json<WebhookInput>,
) -> (StatusCode, Json<ApiResponse<Webhook>>) {
    if let Err(msg) = validate(&input) {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse::error(StatusCode::BAD_REQUEST, msg)));
    }

    let hook = Webhook {
        id: uuid::Uuid::new_v4().to_string(),
        name: input.name,
        url: input.url,
        events: input.events,
        topic_filter: input.topic_filter.filter(|s| !s.is_empty()),
        headers: input.headers.into_iter().filter(|h| !h.key.trim().is_empty()).collect(),
        enabled: input.enabled,
        secret: input.secret.filter(|s| !s.is_empty()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    match state.storage.webhook.upsert(&hook) {
        Ok(()) => {
            state.webhook.reload();
            (StatusCode::CREATED, Json(ApiResponse::success(hook, "webhook created")))
        }
        Err(e) => internal::<Webhook>(e.to_string()),
    }
}

/// GET /api/v1/webhooks/:id — fetch one webhook.
pub async fn get_webhook(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<Webhook>>) {
    match state.storage.webhook.get(&id) {
        Ok(Some(hook)) => (StatusCode::OK, Json(ApiResponse::success(hook, "ok"))),
        Ok(None) => not_found::<Webhook>(),
        Err(e) => internal::<Webhook>(e.to_string()),
    }
}

/// PUT /api/v1/webhooks/:id — update a webhook (preserves id & created_at).
pub async fn update_webhook(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(input): Json<WebhookInput>,
) -> (StatusCode, Json<ApiResponse<Webhook>>) {
    if let Err(msg) = validate(&input) {
        return (StatusCode::BAD_REQUEST, Json(ApiResponse::error(StatusCode::BAD_REQUEST, msg)));
    }

    let existing = match state.storage.webhook.get(&id) {
        Ok(Some(hook)) => hook,
        Ok(None) => return not_found::<Webhook>(),
        Err(e) => return internal::<Webhook>(e.to_string()),
    };

    let hook = Webhook {
        id: existing.id,
        created_at: existing.created_at,
        name: input.name,
        url: input.url,
        events: input.events,
        topic_filter: input.topic_filter.filter(|s| !s.is_empty()),
        headers: input.headers.into_iter().filter(|h| !h.key.trim().is_empty()).collect(),
        enabled: input.enabled,
        secret: input.secret.filter(|s| !s.is_empty()),
    };

    match state.storage.webhook.upsert(&hook) {
        Ok(()) => {
            state.webhook.reload();
            (StatusCode::OK, Json(ApiResponse::success(hook, "webhook updated")))
        }
        Err(e) => internal::<Webhook>(e.to_string()),
    }
}

/// DELETE /api/v1/webhooks/:id — remove a webhook.
pub async fn delete_webhook(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    match state.storage.webhook.delete(&id) {
        Ok(true) => {
            state.webhook.reload();
            (StatusCode::OK, Json(ApiResponse::success(id, "webhook deleted")))
        }
        Ok(false) => not_found::<String>(),
        Err(e) => internal::<String>(e.to_string()),
    }
}

/// POST /api/v1/webhooks/:id/test — send a sample event to the webhook URL.
pub async fn test_webhook(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ApiResponse<String>>) {
    let hook = match state.storage.webhook.get(&id) {
        Ok(Some(hook)) => hook,
        Ok(None) => return not_found::<String>(),
        Err(e) => return internal::<String>(e.to_string()),
    };

    let mut ev = WebhookEvent::new(events::CLIENT_CONNECTED);
    ev.client_id = Some("test-client".to_string());
    ev.username = Some("test".to_string());
    ev.topic = Some("coremq/test".to_string());

    match state.webhook.deliver_once(&hook, &ev).await {
        Ok(code) => (
            StatusCode::OK,
            Json(ApiResponse::success(format!("HTTP {code}"), "test delivered")),
        ),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(ApiResponse::error(StatusCode::BAD_GATEWAY, format!("delivery failed: {e}"))),
        ),
    }
}

fn validate(input: &WebhookInput) -> Result<(), String> {
    if input.name.trim().is_empty() {
        return Err("name is required".into());
    }
    if !(input.url.starts_with("http://") || input.url.starts_with("https://")) {
        return Err("url must start with http:// or https://".into());
    }
    if input.events.is_empty() {
        return Err("at least one event is required".into());
    }
    for e in &input.events {
        if !events::ALL.contains(&e.as_str()) {
            return Err(format!("unknown event: {e}"));
        }
    }
    Ok(())
}

fn not_found<T>() -> (StatusCode, Json<ApiResponse<T>>) {
    (StatusCode::NOT_FOUND, Json(ApiResponse::error(StatusCode::NOT_FOUND, "webhook not found")))
}

fn internal<T>(msg: String) -> (StatusCode, Json<ApiResponse<T>>) {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error(StatusCode::INTERNAL_SERVER_ERROR, msg)))
}
