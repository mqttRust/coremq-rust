use std::fmt;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

/*
  Stable identity of a broker process across restarts. Ordering matters: the
  lexicographically smaller id wins deterministic tiebreaks (duplicate peer
  connections, federation link ownership).
*/
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/*
  What a node advertises about itself in Hello and in membership gossip.

  `incarnation` increments on every process start. It is the tiebreaker that
  lets peers recognise a node that restarted faster than their failure detector
  could notice, and it is what a node bumps to refute a false suspicion.
*/
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDescriptor {
    pub id: NodeId,
    pub cluster: String,
    pub advertise_addr: SocketAddr,
    pub api_addr: Option<SocketAddr>,
    pub incarnation: u64,
    pub started_at: i64,
    pub version: String,
}

impl NodeDescriptor {
    pub fn is_same_process(&self, other: &NodeDescriptor) -> bool {
        self.id == other.id && self.incarnation == other.incarnation
    }
}

/*
  Why a node left. A graceful leave skips the Suspect phase entirely, so routes
  are purged immediately instead of after dead_after.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaveReason {
    Shutdown,
    Evicted,
    ClusterMismatch,
}

impl fmt::Display for LeaveReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            LeaveReason::Shutdown => "shutdown",
            LeaveReason::Evicted => "evicted",
            LeaveReason::ClusterMismatch => "cluster mismatch",
        };
        write!(f, "{}", s)
    }
}
