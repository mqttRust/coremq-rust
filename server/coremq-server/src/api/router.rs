use axum::{Router, middleware, routing::{delete, get, post}};
use tower_http::cors::{Any, CorsLayer};

use crate::api::{
    api_state::ApiState,
    controllers::{sessions, listeners, users, topics, metrics, static_files},
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
            .route("/api/v1/listeners", get(listeners::get_listeners))
            .route("/api/v1/listeners/:port", delete(listeners::stop_listener))
            .route("/api/v1/ws/metrics", get(metrics::ws_metrics))
            .layer(middleware::from_fn_with_state(state.clone(), auth::casbin::auth_middleware));

        Router::new()
            .merge(protected)
            // Public auth routes — no middleware
            .nest("/api/v1/public", self.auth_routes())
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

    fn cors(&self) -> CorsLayer {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    }
}
