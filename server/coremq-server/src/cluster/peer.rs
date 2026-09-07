use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, split};
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc};

use crate::cluster::node::{NodeDescriptor, NodeId};
use crate::cluster::protocol::{ClusterMessage, PROTOCOL_VERSION, read_frame, write_frame};
use crate::models::config::cluster::FederationConfig;

/*
  Outbound queue depth per peer. Forward is a hot path, so this is bounded:
  blocking the cluster runtime on one slow peer would stall every other peer.
  A full queue drops the message and increments a counter instead.
*/
pub const PEER_QUEUE_DEPTH: usize = 1024;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /* We dialled them. */
    Outbound,
    /* They dialled us. */
    Inbound,
}

/*
  A live peer connection as the runtime sees it: a descriptor and a bounded
  sender into the writer task.
*/
pub struct PeerHandle {
    pub desc: NodeDescriptor,
    pub direction: Direction,
    pub tx: mpsc::Sender<ClusterMessage>,
    pub addr: SocketAddr,
    pub dropped: u64,
}

impl PeerHandle {
    /*
      Non-blocking send. Returns false when the peer's queue is full, which the
      runtime records as a drop rather than waiting.
    */
    pub fn try_send(&mut self, msg: ClusterMessage) -> bool {
        match self.tx.try_send(msg) {
            Ok(()) => true,
            Err(_) => {
                self.dropped += 1;
                false
            }
        }
    }

    /*
      Which of two competing connections to keep.

      Both sides run this and reach the same answer: the connection dialled by
      the lexicographically smaller node id wins. Without a deterministic rule a
      simultaneous dial leaves two half-used links.
    */
    pub fn preferred_direction(self_id: &NodeId, peer_id: &NodeId) -> Direction {
        if self_id < peer_id {
            Direction::Outbound
        } else {
            Direction::Inbound
        }
    }
}

/* What a completed handshake hands back to the runtime. */
pub struct Established {
    pub desc: NodeDescriptor,
    pub direction: Direction,
    pub addr: SocketAddr,
    pub tx: mpsc::Sender<ClusterMessage>,
}

/*
  Events a peer's reader task feeds into the runtime.
*/
#[derive(Debug)]
pub enum PeerEvent {
    Frame(NodeId, ClusterMessage),
    Disconnected(NodeId, Direction),
}

pub struct HandshakeResult {
    pub established: Established,
    pub reader: ReaderParts,
}

pub struct ReaderParts {
    pub stream: Box<dyn AsyncRead + Send + Unpin>,
    pub peer_id: NodeId,
    pub direction: Direction,
}

/*
  A federation link opened by the remote cluster. Carries no node identity —
  federated clusters exchange traffic, not membership.
*/
pub struct FederationInbound {
    pub link: String,
    pub remote_cluster: String,
    pub remote_accept: Vec<String>,
    pub tx: mpsc::Sender<ClusterMessage>,
    pub stream: Box<dyn AsyncRead + Send + Unpin>,
}

pub enum Accepted {
    Peer(Box<HandshakeResult>),
    Federation(Box<FederationInbound>),
    Rejected,
}

/*
  Registry of configured federation links, shared with the accept loop so an
  inbound link can be validated against its own secret and filters. The runtime
  updates it whenever links are added or removed through the API.
*/
pub type FederationRegistry = Arc<RwLock<HashMap<String, FederationConfig>>>;

/*
  Dial a peer and complete the handshake.

  Returns Ok(None) when the peer rejected us for a non-fatal reason (cluster
  name mismatch, bad secret) so the caller can log and move on without treating
  it as a transport error.

  The dialer always speaks first; the acceptor always reads first. That fixed
  order is what keeps a simultaneous dial from deadlocking.
*/
pub async fn dial(
    addr: SocketAddr,
    self_desc: &NodeDescriptor,
    secret: &str,
) -> anyhow::Result<Option<HandshakeResult>> {
    let stream = tokio::time::timeout(HANDSHAKE_TIMEOUT, TcpStream::connect(addr)).await??;
    stream.set_nodelay(true)?;

    let (mut read_half, mut write_half) = split(stream);

    let hello = ClusterMessage::Hello {
        node: self_desc.clone(),
        protocol_version: PROTOCOL_VERSION,
        secret: secret.to_string(),
    };
    write_frame(&mut write_half, &hello).await?;

    let peer_hello = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_frame(&mut read_half)).await??;
    let (peer_desc, peer_version, peer_secret) = match peer_hello {
        ClusterMessage::Hello {
            node,
            protocol_version,
            secret,
        } => (node, protocol_version, secret),
        other => anyhow::bail!("expected Hello from {}, got {:?}", addr, other),
    };

    let rejection = validate(self_desc, secret, &peer_desc, peer_version, &peer_secret);
    write_frame(
        &mut write_half,
        &ClusterMessage::HelloAck {
            node: self_desc.clone(),
            accepted: rejection.is_none(),
            reason: rejection.clone(),
        },
    )
    .await?;

    if let Some(reason) = rejection {
        println!("cluster: rejected peer {} ({})", addr, reason);
        return Ok(None);
    }

    let peer_ack = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_frame(&mut read_half)).await??;
    if let ClusterMessage::HelloAck {
        accepted: false,
        reason,
        ..
    } = peer_ack
    {
        println!(
            "cluster: peer {} rejected us ({})",
            addr,
            reason.unwrap_or_else(|| "no reason given".into())
        );
        return Ok(None);
    }

    Ok(Some(finish_peer(
        peer_desc,
        write_half,
        read_half,
        addr,
        Direction::Outbound,
    )))
}

/*
  Complete the handshake on a connection someone else dialled.

  The first frame decides what this connection is: a Hello makes it a cluster
  peer, a FederationHello makes it a link from another cluster.
*/
pub async fn accept(
    stream: TcpStream,
    addr: SocketAddr,
    self_desc: &NodeDescriptor,
    secret: &str,
    federation: &FederationRegistry,
) -> anyhow::Result<Accepted> {
    stream.set_nodelay(true)?;
    let (mut read_half, mut write_half) = split(stream);

    let first = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_frame(&mut read_half)).await??;

    match first {
        ClusterMessage::Hello {
            node: peer_desc,
            protocol_version,
            secret: peer_secret,
        } => {
            let rejection = validate(self_desc, secret, &peer_desc, protocol_version, &peer_secret);

            write_frame(
                &mut write_half,
                &ClusterMessage::Hello {
                    node: self_desc.clone(),
                    protocol_version: PROTOCOL_VERSION,
                    secret: secret.to_string(),
                },
            )
            .await?;

            /* The dialer's HelloAck arrives before ours goes out. */
            let peer_ack =
                tokio::time::timeout(HANDSHAKE_TIMEOUT, read_frame(&mut read_half)).await??;

            write_frame(
                &mut write_half,
                &ClusterMessage::HelloAck {
                    node: self_desc.clone(),
                    accepted: rejection.is_none(),
                    reason: rejection.clone(),
                },
            )
            .await?;

            if let Some(reason) = rejection {
                println!("cluster: rejected peer {} ({})", addr, reason);
                return Ok(Accepted::Rejected);
            }

            if let ClusterMessage::HelloAck {
                accepted: false,
                reason,
                ..
            } = peer_ack
            {
                println!(
                    "cluster: peer {} rejected us ({})",
                    addr,
                    reason.unwrap_or_else(|| "no reason given".into())
                );
                return Ok(Accepted::Rejected);
            }

            Ok(Accepted::Peer(Box::new(finish_peer(
                peer_desc,
                write_half,
                read_half,
                addr,
                Direction::Inbound,
            ))))
        }

        ClusterMessage::FederationHello {
            cluster,
            link,
            secret: remote_secret,
            accept,
            protocol_version,
        } => {
            let configured = federation.read().await.get(&link).cloned();

            let rejection = if protocol_version != PROTOCOL_VERSION {
                Some(format!("protocol version {} unsupported", protocol_version))
            } else if cluster == self_desc.cluster {
                Some("that cluster name is our own; a link to ourselves is a loop".to_string())
            } else {
                match &configured {
                    None => Some(format!("no federation link named '{}' is configured", link)),
                    Some(cfg) if !cfg.secret.is_empty() && cfg.secret != remote_secret => {
                        Some("shared secret mismatch".to_string())
                    }
                    Some(_) => None,
                }
            };

            write_frame(
                &mut write_half,
                &ClusterMessage::FederationAck {
                    cluster: self_desc.cluster.clone(),
                    accepted: rejection.is_none(),
                    reason: rejection.clone(),
                },
            )
            .await?;

            if let Some(reason) = rejection {
                println!("cluster: rejected federation link from {} ({})", addr, reason);
                return Ok(Accepted::Rejected);
            }

            let (tx, rx) = mpsc::channel::<ClusterMessage>(PEER_QUEUE_DEPTH);
            tokio::spawn(async move {
                writer_loop(write_half, rx).await;
            });

            Ok(Accepted::Federation(Box::new(FederationInbound {
                link,
                remote_cluster: cluster,
                remote_accept: accept,
                tx,
                stream: Box::new(read_half),
            })))
        }

        other => anyhow::bail!("unexpected opening frame from {}: {:?}", addr, other),
    }
}

fn finish_peer<W, R>(
    peer_desc: NodeDescriptor,
    write_half: W,
    read_half: R,
    addr: SocketAddr,
    direction: Direction,
) -> HandshakeResult
where
    W: AsyncWrite + Unpin + Send + 'static,
    R: AsyncRead + Unpin + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<ClusterMessage>(PEER_QUEUE_DEPTH);
    let peer_id = peer_desc.id.clone();

    tokio::spawn(async move {
        writer_loop(write_half, rx).await;
    });

    HandshakeResult {
        established: Established {
            desc: peer_desc,
            direction,
            addr,
            tx,
        },
        reader: ReaderParts {
            stream: Box::new(read_half),
            peer_id,
            direction,
        },
    }
}

fn validate(
    self_desc: &NodeDescriptor,
    self_secret: &str,
    peer: &NodeDescriptor,
    peer_version: u16,
    peer_secret: &str,
) -> Option<String> {
    if peer_version != PROTOCOL_VERSION {
        return Some(format!(
            "protocol version {} does not match ours ({})",
            peer_version, PROTOCOL_VERSION
        ));
    }

    /*
      The cluster name is the guard against accidentally joining staging nodes
      to production.
    */
    if peer.cluster != self_desc.cluster {
        return Some(format!(
            "cluster '{}' does not match ours ('{}')",
            peer.cluster, self_desc.cluster
        ));
    }

    if !self_secret.is_empty() && self_secret != peer_secret {
        return Some("shared secret mismatch".to_string());
    }

    if peer.id == self_desc.id {
        return Some("peer advertises our own node id".to_string());
    }

    None
}

async fn writer_loop<W>(mut writer: W, mut rx: mpsc::Receiver<ClusterMessage>)
where
    W: AsyncWrite + Unpin,
{
    while let Some(msg) = rx.recv().await {
        if let Err(e) = write_frame(&mut writer, &msg).await {
            println!("cluster: peer write failed: {}", e);
            break;
        }
    }
}

/*
  Pump frames from a peer into the runtime until the connection dies. Always
  emits Disconnected on exit so the runtime can never leak a peer entry.
*/
pub async fn reader_loop(mut parts: ReaderParts, events: mpsc::Sender<PeerEvent>) {
    loop {
        match read_frame(&mut parts.stream).await {
            Ok(msg) => {
                if events
                    .send(PeerEvent::Frame(parts.peer_id.clone(), msg))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let _ = events
        .send(PeerEvent::Disconnected(parts.peer_id, parts.direction))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sides_agree_on_which_connection_to_keep() {
        let a = NodeId::new("node-a");
        let b = NodeId::new("node-b");

        /*
          a is smaller, so the link a dialled wins. From a's view that is its
          outbound; from b's view the same link is its inbound.
        */
        assert_eq!(PeerHandle::preferred_direction(&a, &b), Direction::Outbound);
        assert_eq!(PeerHandle::preferred_direction(&b, &a), Direction::Inbound);
    }

    fn desc(id: &str, cluster: &str) -> NodeDescriptor {
        NodeDescriptor {
            id: NodeId::new(id),
            cluster: cluster.into(),
            advertise_addr: "127.0.0.1:4370".parse().unwrap(),
            api_addr: None,
            incarnation: 1,
            started_at: 0,
            version: "0".into(),
        }
    }

    #[test]
    fn cluster_name_mismatch_is_rejected() {
        let me = desc("a", "prod");
        let them = desc("b", "staging");

        let reason = validate(&me, "", &them, PROTOCOL_VERSION, "");
        assert!(reason.unwrap().contains("does not match"));
    }

    #[test]
    fn protocol_version_mismatch_is_rejected() {
        let me = desc("a", "prod");
        let them = desc("b", "prod");

        assert!(validate(&me, "", &them, PROTOCOL_VERSION + 1, "").is_some());
    }

    #[test]
    fn wrong_secret_is_rejected_but_a_blank_one_accepts_anything() {
        let me = desc("a", "prod");
        let them = desc("b", "prod");

        assert!(validate(&me, "hunter2", &them, PROTOCOL_VERSION, "wrong").is_some());
        assert!(validate(&me, "hunter2", &them, PROTOCOL_VERSION, "hunter2").is_none());
        assert!(validate(&me, "", &them, PROTOCOL_VERSION, "anything").is_none());
    }

    #[test]
    fn a_peer_claiming_our_id_is_rejected() {
        let me = desc("a", "prod");
        let them = desc("a", "prod");

        assert!(validate(&me, "", &them, PROTOCOL_VERSION, "").is_some());
    }
}
