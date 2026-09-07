use std::net::SocketAddr;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

use crate::cluster::federation::LinkState;
use crate::cluster::node::NodeId;
use crate::models::config::cluster::FederationConfig;
use crate::protocol::packets::PublishPacket;

/*
  Where a publish came from. The loop guard depends on this: only a message that
  arrived from a local client is forwarded onward.
*/
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Local,
    Remote(NodeId),
    Federation(String),
}

impl Origin {
    pub fn is_local(&self) -> bool {
        matches!(self, Origin::Local)
    }
}

/*
  Depth of the engine -> cluster queue. Bounded because Publish rides it; a
  wedged cluster runtime must never stall the engine actor.
*/
pub const CLUSTER_QUEUE_DEPTH: usize = 4096;

#[derive(Debug)]
pub enum ClusterRequest {
    /* First local subscriber for this filter. */
    RouteAdd(String),
    /* Last local subscriber for this filter went away. */
    RouteDel(String),
    /* A locally-originated publish that may need forwarding. */
    Forward(PublishPacket),
    /* A local client connected; claim the id cluster-wide. */
    ClaimSession(String),

    Status(oneshot::Sender<ClusterStatus>),
    Nodes(oneshot::Sender<Vec<NodeView>>),
    Routes(oneshot::Sender<Vec<RouteView>>),
    Sessions(oneshot::Sender<Vec<RemoteSession>>),
    Join(SocketAddr, oneshot::Sender<Result<(), String>>),
    Evict(NodeId, oneshot::Sender<bool>),

    FederationStatus(oneshot::Sender<Vec<FederationView>>),
    FederationAdd(Box<FederationConfig>, oneshot::Sender<Result<(), String>>),
    FederationRemove(String, oneshot::Sender<bool>),

    Shutdown(oneshot::Sender<()>),
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterStatus {
    pub enabled: bool,
    pub cluster: String,
    pub node_id: String,
    pub advertise_addr: String,
    pub incarnation: u64,
    pub discovery: String,
    pub members_total: usize,
    pub members_alive: usize,
    pub routes_total: usize,
    pub federation_links: usize,
    pub is_federation_owner: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeView {
    pub id: String,
    pub cluster: String,
    pub advertise_addr: String,
    pub api_addr: Option<String>,
    pub state: String,
    pub incarnation: u64,
    pub version: String,
    /* Seconds since we last heard from this node. */
    pub last_seen_secs: u64,
    pub is_self: bool,
    pub routes: usize,
    pub dropped_messages: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteView {
    pub filter: String,
    pub node: String,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct RemoteSession {
    pub client_id: String,
    pub username: String,
    pub node: String,
    pub remote_addr: String,
    pub connected_port: u16,
    pub connected_at: String,
    pub subscriptions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FederationView {
    pub name: String,
    pub endpoints: Vec<String>,
    pub forward: Vec<String>,
    pub accept: Vec<String>,
    pub state: LinkState,
    pub sent: u64,
    pub received: u64,
    pub dropped: u64,
    pub last_error: Option<String>,
}

/*
  Cheap-to-clone handle held by the engine and the admin API.

  Every send is non-blocking. A full queue drops the request rather than
  applying backpressure to the MQTT hot path, and the periodic reconcile
  repairs anything a dropped route delta would have missed.
*/
#[derive(Clone)]
pub struct ClusterHandle {
    inner: Arc<HandleInner>,
}

struct HandleInner {
    tx: mpsc::Sender<ClusterRequest>,
    node_id: NodeId,
    cluster: String,
}

impl ClusterHandle {
    pub fn new(tx: mpsc::Sender<ClusterRequest>, node_id: NodeId, cluster: String) -> Self {
        Self {
            inner: Arc::new(HandleInner {
                tx,
                node_id,
                cluster,
            }),
        }
    }

    pub fn node_id(&self) -> &NodeId {
        &self.inner.node_id
    }

    pub fn cluster(&self) -> &str {
        &self.inner.cluster
    }

    pub fn route_add(&self, filter: &str) {
        self.dispatch(ClusterRequest::RouteAdd(filter.to_string()));
    }

    pub fn route_del(&self, filter: &str) {
        self.dispatch(ClusterRequest::RouteDel(filter.to_string()));
    }

    pub fn forward(&self, packet: PublishPacket) {
        self.dispatch(ClusterRequest::Forward(packet));
    }

    pub fn claim_session(&self, client_id: &str) {
        self.dispatch(ClusterRequest::ClaimSession(client_id.to_string()));
    }

    pub fn send(&self, req: ClusterRequest) -> bool {
        self.inner.tx.try_send(req).is_ok()
    }

    fn dispatch(&self, req: ClusterRequest) {
        if self.inner.tx.try_send(req).is_err() {
            println!("cluster: request queue full, dropping a cluster update");
        }
    }
}
