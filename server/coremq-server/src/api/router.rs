use axum::{Router, middleware, routing::{delete, get, post}};
use tower_http::cors::{Any, CorsLayer};

use crate::api::{
    api_state::ApiState,
    controllers::{sessions, listeners, users, topics, metrics, webhooks, tls, mqtt_auth, cluster, static_files},
    auth,
};

pub struct RouterHandler {}

impl RouterHandler {
    pub fn new() -> Self {
        RouterHandler {}
    }

    pub fn create_router(&self, state: ApiState) -> Router {
        // Protected API routes — auth middleware applied only to these
        let protected = Router::new()
            .nest("/api/v1", self.get_session_routes())
            .nest("/api/v1", self.get_user_routes())
            .nest("/api/v1", self.get_topic_routes())
            .nest("/api/v1", self.get_webhook_routes())
            .route("/api/v1/tls/generate", post(tls::generate_cert))
            .nest("/api/v1", self.get_mqtt_auth_routes())
            .nest("/api/v1", self.get_cluster_routes())
            .route("/api/v1/listeners", get(listeners::get_listeners).post(listeners::create_listener))
            .route("/api/v1/listeners/:port", delete(listeners::stop_listener).put(listeners::update_listener))
            .route("/api/v1/ws/metrics", get(metrics::ws_metrics))
            .layer(middleware::from_fn_with_state(state.clone(), auth::casbin::auth_middleware));

        Router::new()
            .merge(protected)
            // Public auth routes — no middleware
            .nest("/api/v1/public", self.auth_routes())
            // Token refresh — auth middleware skips the /api/v1/auth prefix
            .route("/api/v1/auth/refresh-token", post(users::refresh_token))
            // Embedded React SPA — fallback serves index.html for client-side routing
            .fallback(static_files::spa_handler)
            .layer(self.cors())
            .with_state(state)
    }

    pub fn auth_routes(&self) -> Router<ApiState> {
        Router::new().route("/login", post(users::login))
    }

    pub fn get_session_routes(&self) -> Router<ApiState> {
        Router::new()
            .route("/sessions", get(sessions::get_sessions))
            .route("/sessions/:client_id", delete(sessions::disconnect_session))
    }

    pub fn get_user_routes(&self) -> Router<ApiState> {
        Router::new().route("/users", post(users::create_user).get(users::get_all_users))
    }

    pub fn get_topic_routes(&self) -> Router<ApiState> {
        Router::new()
            .route("/topics", get(topics::get_topics))
            .route("/publish", post(topics::publish_message))
    }

    pub fn get_mqtt_auth_routes(&self) -> Router<ApiState> {
        Router::new()
            .route("/mqtt-auth/config", get(mqtt_auth::get_config).put(mqtt_auth::update_config))
            .route(
                "/mqtt-auth/credentials",
                get(mqtt_auth::list_credentials).post(mqtt_auth::create_credential),
            )
            .route("/mqtt-auth/credentials/:username", delete(mqtt_auth::delete_credential))
    }

    pub fn get_cluster_routes(&self) -> Router<ApiState> {
        Router::new()
            .route("/cluster", get(cluster::get_status))
            .route(
                "/cluster/nodes",
                get(cluster::get_nodes).post(cluster::join_node),
            )
            .route("/cluster/nodes/:id", delete(cluster::evict_node))
            .route("/cluster/routes", get(cluster::get_routes))
            .route("/cluster/sessions", get(cluster::get_sessions))
            .route(
                "/cluster/federation",
                get(cluster::get_federation).post(cluster::create_federation),
            )
            .route("/cluster/federation/:name", delete(cluster::delete_federation))
    }

    pub fn get_webhook_routes(&self) -> Router<ApiState> {
        Router::new()
            .route("/webhooks", get(webhooks::list_webhooks).post(webhooks::create_webhook))
            .route(
                "/webhooks/:id",
                get(webhooks::get_webhook)
                    .put(webhooks::update_webhook)
                    .delete(webhooks::delete_webhook),
            )
            .route("/webhooks/:id/test", post(webhooks::test_webhook))
    }

    fn cors(&self) -> CorsLayer {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    }
}
