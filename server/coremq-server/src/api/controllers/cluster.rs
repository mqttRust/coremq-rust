use std::net::SocketAddr;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use tokio::sync::oneshot;

use crate::api::api_state::{ApiResponse, ApiState};
use crate::cluster::handle::{
    ClusterRequest, ClusterStatus, FederationView, NodeView, RemoteSession, RouteView,
};
use crate::cluster::node::NodeId;
use crate::models::config::cluster::FederationConfig;

/*
  Ask the cluster runtime a question and wait for its reply.

  Every path here returns a definite answer: a runtime that has gone away
  produces SERVICE_UNAVAILABLE rather than leaving the request hanging.
*/
async fn query<T, F>(state: &ApiState, build: F) -> Result<T, StatusCode>
where
    F: FnOnce(oneshot::Sender<T>) -> ClusterRequest,
{
    let cluster = state.cluster.as_ref().ok_or(StatusCode::NOT_FOUND)?;
    let (tx, rx) = oneshot::channel();

    if !cluster.send(build(tx)) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    rx.await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

/*
  GET /api/v1/cluster
  Reports enabled:false rather than 404 so the dashboard can render a
  "clustering is off" state instead of an error.
*/
pub async fn get_status(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<ClusterStatus>>, StatusCode> {
    let Some(cluster) = state.cluster.as_ref() else {
        return Ok(Json(ApiResponse::success(
            ClusterStatus {
                enabled: false,
                cluster: String::new(),
                node_id: String::new(),
                advertise_addr: String::new(),
                incarnation: 0,
                discovery: String::new(),
                members_total: 0,
                members_alive: 0,
                routes_total: 0,
                federation_links: 0,
                is_federation_owner: false,
            },
            "clustering is disabled",
        )));
    };

    let (tx, rx) = oneshot::channel();
    if !cluster.send(ClusterRequest::Status(tx)) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let status = rx.await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(Json(ApiResponse::success(status, "ok")))
}

/* GET /api/v1/cluster/nodes */
pub async fn get_nodes(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<NodeView>>>, StatusCode> {
    let nodes = query(&state, ClusterRequest::Nodes).await?;
    Ok(Json(ApiResponse::success(nodes, "ok")))
}

/* GET /api/v1/cluster/routes */
pub async fn get_routes(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<RouteView>>>, StatusCode> {
    let routes = query(&state, ClusterRequest::Routes).await?;
    Ok(Json(ApiResponse::success(routes, "ok")))
}

/* GET /api/v1/cluster/sessions — every node's sessions, merged. */
pub async fn get_sessions(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<RemoteSession>>>, StatusCode> {
    let sessions = query(&state, ClusterRequest::Sessions).await?;
    Ok(Json(ApiResponse::success(sessions, "ok")))
}

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub address: String,
}

/* POST /api/v1/cluster/nodes — dial a peer by address. */
pub async fn join_node(
    State(state): State<ApiState>,
    Json(body): Json<JoinRequest>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let addr: SocketAddr = body
        .address
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let result = query(&state, |tx| ClusterRequest::Join(addr, tx)).await?;

    match result {
        Ok(()) => Ok(Json(ApiResponse::success(
            body.address,
            "dialling the peer",
        ))),
        Err(e) => Ok(Json(ApiResponse::error(StatusCode::CONFLICT, e))),
    }
}

/*
  DELETE /api/v1/cluster/nodes/:id
  Force-removes a node the failure detector has already given up on.
*/
pub async fn evict_node(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let node = NodeId::new(id.clone());
    let existed = query(&state, |tx| ClusterRequest::Evict(node, tx)).await?;

    if existed {
        Ok(Json(ApiResponse::success(id, "node evicted")))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/* GET /api/v1/cluster/federation */
pub async fn get_federation(
    State(state): State<ApiState>,
) -> Result<Json<ApiResponse<Vec<FederationView>>>, StatusCode> {
    let links = query(&state, ClusterRequest::FederationStatus).await?;
    Ok(Json(ApiResponse::success(links, "ok")))
}

/* POST /api/v1/cluster/federation */
pub async fn create_federation(
    State(state): State<ApiState>,
    Json(body): Json<FederationConfig>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let name = body.name.clone();
    let result = query(&state, |tx| {
        ClusterRequest::FederationAdd(Box::new(body), tx)
    })
    .await?;

    match result {
        Ok(()) => Ok(Json(ApiResponse::success(name, "federation link created"))),
        Err(e) => Ok(Json(ApiResponse::error(StatusCode::CONFLICT, e))),
    }
}

/* DELETE /api/v1/cluster/federation/:name */
pub async fn delete_federation(
    State(state): State<ApiState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let removed = query(&state, |tx| {
        ClusterRequest::FederationRemove(name.clone(), tx)
    })
    .await?;

    if removed {
        Ok(Json(ApiResponse::success(name, "federation link removed")))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
