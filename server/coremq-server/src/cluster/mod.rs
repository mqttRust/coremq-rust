pub mod discovery;
pub mod federation;
pub mod federation_link;
pub mod handle;
pub mod membership;
pub mod meta;
pub mod node;
pub mod peer;
pub mod protocol;
pub mod router;
pub mod runtime;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use tokio::sync::mpsc;

pub use handle::{ClusterHandle, ClusterRequest, Origin};

use crate::cluster::meta::Replication;
use crate::cluster::node::{NodeDescriptor, NodeId};
use crate::models::config::cluster::ClusterConfig;
use crate::storage::redb::cluster::ClusterRepo;

/*
  Resolve this node's identity.

  Order: explicit config, then COREMQ_NODE_ID, then whatever we persisted on a
  previous boot, and only then a fresh UUID which is immediately persisted. A
  node that loses its id looks brand new to every peer, which is survivable but
  forces a full route resync.
*/
pub fn resolve_identity(
    config: &ClusterConfig,
    repo: &ClusterRepo,
    api_addr: Option<SocketAddr>,
) -> anyhow::Result<NodeDescriptor> {
    let configured = config.node_id.trim();

    let id = if !configured.is_empty() {
        configured.to_string()
    } else if let Ok(from_env) = std::env::var("COREMQ_NODE_ID") {
        let trimmed = from_env.trim().to_string();
        if trimmed.is_empty() {
            persisted_or_new(repo)?
        } else {
            trimmed
        }
    } else {
        persisted_or_new(repo)?
    };

    repo.set_node_id(&id)?;
    let incarnation = repo.bump_incarnation()?;

    let bind: SocketAddr = config
        .bind
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid cluster.bind '{}'", config.bind))?;

    let advertise = resolve_advertise(config, bind)?;

    Ok(NodeDescriptor {
        id: NodeId::new(id),
        cluster: config.name.clone(),
        advertise_addr: advertise,
        api_addr,
        incarnation,
        started_at: crate::cluster::meta::now_ms(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

fn persisted_or_new(repo: &ClusterRepo) -> anyhow::Result<String> {
    if let Some(existing) = repo.get_node_id()? {
        if !existing.trim().is_empty() {
            return Ok(existing);
        }
    }
    Ok(uuid::Uuid::new_v4().to_string())
}

/*
  Work out the address peers should dial.

  An explicit `advertise` always wins. Otherwise the bind address is used, and a
  wildcard bind is rewritten to the first non-loopback interface — advertising
  0.0.0.0 would tell peers to dial a meaningless address.
*/
fn resolve_advertise(config: &ClusterConfig, bind: SocketAddr) -> anyhow::Result<SocketAddr> {
    let configured = config.advertise.trim();
    if !configured.is_empty() {
        return configured
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid cluster.advertise '{}'", configured));
    }

    if !bind.ip().is_unspecified() {
        return Ok(bind);
    }

    match local_ipv4() {
        Some(ip) => Ok(SocketAddr::new(IpAddr::V4(ip), bind.port())),
        None => {
            println!(
                "cluster: could not determine a routable address; advertising {}. \
                 Set cluster.advertise if peers cannot reach this node.",
                bind
            );
            Ok(bind)
        }
    }
}

/*
  Best-effort local address discovery without a dependency: open a UDP socket
  towards a public address and read back the interface the OS chose. No packet
  is actually sent.
*/
fn local_ipv4() -> Option<Ipv4Addr> {
    let socket = std::net::UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect(("8.8.8.8", 53)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/*
  Build the replication hook the storage repos use to stamp and announce writes.
*/
pub fn build_replication(
    node_id: NodeId,
    repo: ClusterRepo,
) -> (Arc<Replication>, mpsc::Receiver<crate::cluster::protocol::MetaEntry>) {
    Replication::new(node_id, repo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with(bind: &str, advertise: &str) -> ClusterConfig {
        ClusterConfig {
            bind: bind.to_string(),
            advertise: advertise.to_string(),
            ..ClusterConfig::default()
        }
    }

    #[test]
    fn explicit_advertise_wins() {
        let cfg = config_with("0.0.0.0:4370", "10.0.0.5:4370");
        let bind: SocketAddr = "0.0.0.0:4370".parse().unwrap();

        assert_eq!(
            resolve_advertise(&cfg, bind).unwrap(),
            "10.0.0.5:4370".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn a_specific_bind_is_advertised_as_is() {
        let cfg = config_with("192.168.1.10:4370", "");
        let bind: SocketAddr = "192.168.1.10:4370".parse().unwrap();

        assert_eq!(resolve_advertise(&cfg, bind).unwrap(), bind);
    }

    #[test]
    fn a_wildcard_bind_is_never_advertised_verbatim_when_an_interface_exists() {
        let cfg = config_with("0.0.0.0:4370", "");
        let bind: SocketAddr = "0.0.0.0:4370".parse().unwrap();

        let resolved = resolve_advertise(&cfg, bind).unwrap();
        assert_eq!(resolved.port(), 4370);

        /* Either a real interface was found, or we fell back with a warning. */
        if local_ipv4().is_some() {
            assert!(!resolved.ip().is_unspecified());
        }
    }

    #[test]
    fn a_bad_advertise_string_is_an_error() {
        let cfg = config_with("0.0.0.0:4370", "not-an-address");
        let bind: SocketAddr = "0.0.0.0:4370".parse().unwrap();

        assert!(resolve_advertise(&cfg, bind).is_err());
    }
}
