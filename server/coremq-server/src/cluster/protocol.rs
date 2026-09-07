use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::cluster::node::{LeaveReason, NodeDescriptor, NodeId};
use crate::protocol::packets::PublishPacket;

/*
  Bumped on any breaking change to ClusterMessage. Mismatched versions are
  rejected during the handshake rather than left to misparse.
*/
pub const PROTOCOL_VERSION: u16 = 1;

/*
  Hard cap on a single frame. A larger length prefix is treated as a protocol
  violation and closes the connection instead of allocating.
*/
pub const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;

/*
  A publish as it crosses the wire.

  packet_id is deliberately absent: MQTT packet ids are scoped to a single
  client connection, so the receiving node assigns its own before delivery.
  Forwarding the origin's id would collide with the receiver's id space.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WirePublish {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub retain: bool,
    pub dup: bool,
}

impl From<&PublishPacket> for WirePublish {
    fn from(p: &PublishPacket) -> Self {
        Self {
            topic: p.topic.clone(),
            payload: p.payload.clone(),
            qos: p.qos,
            retain: p.retain,
            dup: p.dup,
        }
    }
}

impl WirePublish {
    /*
      Rebuild a deliverable packet. `packet_id` must be Some for QoS > 0 —
      encode_publish panics on a None id, so the caller allocates one from the
      node-local counter.
    */
    pub fn into_packet(self, packet_id: Option<u16>) -> PublishPacket {
        PublishPacket {
            packet_id: if self.qos > 0 { packet_id } else { None },
            topic: self.topic,
            payload: self.payload,
            qos: self.qos,
            retain: self.retain,
            dup: self.dup,
        }
    }
}

/*
  One node's view of another, as gossiped around the mesh.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberEntry {
    pub desc: NodeDescriptor,
    pub state: WireMemberState,
    pub incarnation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireMemberState {
    Alive,
    Suspect,
    Dead,
    Left,
}

/*
  Which replicated table a metadata delta belongs to. Kept as an explicit enum
  rather than a string so an unknown table is a decode error, not a silent
  write into the wrong place.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetaTable {
    User,
    Listener,
    Webhook,
    AuthConfig,
    Credential,
}

impl MetaTable {
    pub fn as_str(&self) -> &'static str {
        match self {
            MetaTable::User => "user",
            MetaTable::Listener => "listener",
            MetaTable::Webhook => "webhook",
            MetaTable::AuthConfig => "auth_config",
            MetaTable::Credential => "credential",
        }
    }
}

/*
  A last-write-wins metadata record. `value: None` is a tombstone — a hard
  delete would be resurrected by any peer still holding the old value.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaEntry {
    pub table: MetaTable,
    pub key: String,
    pub value: Option<Vec<u8>>,
    pub updated_at: i64,
    pub updated_by: NodeId,
}

impl MetaEntry {
    /*
      LWW comparison. Timestamps decide; the node id breaks exact ties so every
      node converges on the same winner regardless of arrival order.
    */
    pub fn supersedes(&self, other: &MetaEntry) -> bool {
        match self.updated_at.cmp(&other.updated_at) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => self.updated_by > other.updated_by,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterMessage {
    Hello {
        node: NodeDescriptor,
        protocol_version: u16,
        /* Blank when cluster.secret is unset. */
        secret: String,
    },
    HelloAck {
        node: NodeDescriptor,
        accepted: bool,
        reason: Option<String>,
    },

    Ping {
        seq: u64,
    },
    Pong {
        seq: u64,
    },
    /* Ask this peer to probe `target` on our behalf. */
    PingReq {
        target: NodeId,
        seq: u64,
    },
    PingAck {
        target: NodeId,
        seq: u64,
        reachable: bool,
    },

    Membership {
        entries: Vec<MemberEntry>,
    },

    RouteAdd {
        node: NodeId,
        filter: String,
        epoch: u64,
    },
    RouteDel {
        node: NodeId,
        filter: String,
        epoch: u64,
    },
    RouteSyncRequest {
        node: NodeId,
        since_epoch: u64,
    },
    RouteSnapshot {
        node: NodeId,
        filters: Vec<String>,
        epoch: u64,
    },

    Forward {
        origin: NodeId,
        packet: WirePublish,
        /*
          Accumulated cluster names. A node drops any message whose path already
          contains its own cluster, which makes federation cycles safe.
        */
        cluster_path: Vec<String>,
    },

    SessionClaim {
        client_id: String,
        node: NodeId,
        claimed_at: i64,
    },

    MetaDelta {
        entries: Vec<MetaEntry>,
    },
    MetaSyncRequest,

    Leave {
        node: NodeId,
        reason: LeaveReason,
    },

    /*
      Federation handshake. Carries the link name and the filters the dialing
      cluster is willing to receive, so the remote side can apply our accept
      list without it being configured twice.
    */
    FederationHello {
        cluster: String,
        link: String,
        secret: String,
        accept: Vec<String>,
        protocol_version: u16,
    },
    FederationAck {
        cluster: String,
        accepted: bool,
        reason: Option<String>,
    },
}

/*
  Frame layout: [len: u32 big-endian][bincode payload].
*/
pub async fn write_frame<W>(writer: &mut W, msg: &ClusterMessage) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let bytes = bincode::serialize(msg)?;
    if bytes.len() > MAX_FRAME_BYTES {
        anyhow::bail!("cluster frame of {} bytes exceeds the cap", bytes.len());
    }

    writer.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_frame<R>(reader: &mut R) -> anyhow::Result<ClusterMessage>
where
    R: AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > MAX_FRAME_BYTES {
        anyhow::bail!("peer announced a {} byte frame, over the cap", len);
    }
    if len == 0 {
        anyhow::bail!("peer sent an empty frame");
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(bincode::deserialize(&buf)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lww_prefers_the_later_timestamp() {
        let older = MetaEntry {
            table: MetaTable::User,
            key: "alice".into(),
            value: Some(vec![1]),
            updated_at: 100,
            updated_by: NodeId::new("node-a"),
        };
        let newer = MetaEntry {
            updated_at: 200,
            ..older.clone()
        };

        assert!(newer.supersedes(&older));
        assert!(!older.supersedes(&newer));
    }

    #[test]
    fn lww_breaks_exact_ties_on_node_id() {
        let from_a = MetaEntry {
            table: MetaTable::User,
            key: "alice".into(),
            value: Some(vec![1]),
            updated_at: 100,
            updated_by: NodeId::new("node-a"),
        };
        let from_b = MetaEntry {
            updated_by: NodeId::new("node-b"),
            ..from_a.clone()
        };

        /* Deterministic in both directions, so every node picks the same winner. */
        assert!(from_b.supersedes(&from_a));
        assert!(!from_a.supersedes(&from_b));
    }

    #[tokio::test]
    async fn frames_round_trip() {
        let msg = ClusterMessage::RouteAdd {
            node: NodeId::new("node-a"),
            filter: "sensors/#".into(),
            epoch: 7,
        };

        let mut buf = Vec::new();
        write_frame(&mut buf, &msg).await.unwrap();

        let mut cursor = std::io::Cursor::new(buf);
        let decoded = read_frame(&mut cursor).await.unwrap();

        match decoded {
            ClusterMessage::RouteAdd { node, filter, epoch } => {
                assert_eq!(node.as_str(), "node-a");
                assert_eq!(filter, "sensors/#");
                assert_eq!(epoch, 7);
            }
            other => panic!("unexpected frame: {:?}", other),
        }
    }

    #[tokio::test]
    async fn oversized_length_prefix_is_rejected() {
        let mut framed = Vec::new();
        framed.extend_from_slice(&(u32::MAX).to_be_bytes());

        let mut cursor = std::io::Cursor::new(framed);
        assert!(read_frame(&mut cursor).await.is_err());
    }

    #[test]
    fn wire_publish_drops_packet_id_for_qos0() {
        let wire = WirePublish {
            topic: "a/b".into(),
            payload: vec![1, 2, 3],
            qos: 0,
            retain: false,
            dup: false,
        };

        let packet = wire.into_packet(Some(42));
        assert_eq!(packet.packet_id, None);
    }

    #[test]
    fn wire_publish_keeps_assigned_id_for_qos1() {
        let wire = WirePublish {
            topic: "a/b".into(),
            payload: vec![],
            qos: 1,
            retain: false,
            dup: false,
        };

        let packet = wire.into_packet(Some(42));
        assert_eq!(packet.packet_id, Some(42));
    }
}
