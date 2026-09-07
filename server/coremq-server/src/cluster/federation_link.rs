use std::time::Duration;

use tokio::io::split;
use tokio::net::{TcpStream, lookup_host};
use tokio::sync::mpsc;

use crate::cluster::peer::PEER_QUEUE_DEPTH;
use crate::cluster::protocol::{ClusterMessage, PROTOCOL_VERSION, read_frame, write_frame};
use crate::cluster::runtime::RuntimeEvent;
use crate::models::config::cluster::FederationConfig;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/*
  Dial a remote cluster and pump allowlisted traffic across it.

  A federation link is deliberately not a peer: it exchanges only Forward frames
  and the handshake. No membership, no metadata, no failure detector. Federated
  clusters share traffic, not identity.
*/
pub async fn connect(
    config: FederationConfig,
    local_cluster: String,
    events: mpsc::Sender<RuntimeEvent>,
) {
    let name = config.name.clone();

    for endpoint in &config.endpoints {
        match try_endpoint(&config, &local_cluster, endpoint, &events).await {
            Ok(()) => return,
            Err(e) => {
                println!(
                    "cluster: federation '{}' could not use {}: {}",
                    name, endpoint, e
                );
            }
        }
    }

    let _ = events
        .send(RuntimeEvent::FederationDown(
            name,
            "no endpoint accepted the connection".to_string(),
        ))
        .await;
}

async fn try_endpoint(
    config: &FederationConfig,
    local_cluster: &str,
    endpoint: &str,
    events: &mpsc::Sender<RuntimeEvent>,
) -> anyhow::Result<()> {
    let mut addrs = lookup_host(endpoint).await?;
    let addr = addrs
        .next()
        .ok_or_else(|| anyhow::anyhow!("'{}' did not resolve", endpoint))?;

    let stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await??;
    stream.set_nodelay(true)?;

    let (mut read_half, mut write_half) = split(stream);

    let hello = ClusterMessage::FederationHello {
        cluster: local_cluster.to_string(),
        link: config.name.clone(),
        secret: config.secret.clone(),
        /*
          Telling the remote what we accept lets it filter at the source instead
          of shipping traffic we would only discard.
        */
        accept: config.accept.clone(),
        protocol_version: PROTOCOL_VERSION,
    };
    write_frame(&mut write_half, &hello).await?;

    let ack = tokio::time::timeout(CONNECT_TIMEOUT, read_frame(&mut read_half)).await??;
    let remote_accept = match ack {
        ClusterMessage::FederationAck {
            accepted: true,
            cluster,
            ..
        } => {
            if cluster == local_cluster {
                anyhow::bail!("remote reports the same cluster name; that is a loop, not a link");
            }
            Vec::new()
        }
        ClusterMessage::FederationAck {
            accepted: false,
            reason,
            ..
        } => {
            anyhow::bail!(
                "remote refused: {}",
                reason.unwrap_or_else(|| "no reason given".into())
            );
        }
        ClusterMessage::FederationHello { accept, cluster, .. } => {
            /*
              The remote opened with its own Hello, which happens when both
              sides dial. Its accept list is what we may send.
            */
            if cluster == local_cluster {
                anyhow::bail!("remote reports the same cluster name; that is a loop, not a link");
            }
            let reply = ClusterMessage::FederationAck {
                cluster: local_cluster.to_string(),
                accepted: true,
                reason: None,
            };
            write_frame(&mut write_half, &reply).await?;
            accept
        }
        other => anyhow::bail!("unexpected federation frame: {:?}", other),
    };

    let (tx, mut rx) = mpsc::channel::<ClusterMessage>(PEER_QUEUE_DEPTH);

    events
        .send(RuntimeEvent::FederationUp(
            config.name.clone(),
            tx,
            remote_accept,
        ))
        .await
        .map_err(|_| anyhow::anyhow!("cluster runtime is gone"))?;

    let writer_name = config.name.clone();
    let writer_events = events.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Err(e) = write_frame(&mut write_half, &msg).await {
                let _ = writer_events
                    .send(RuntimeEvent::FederationDown(
                        writer_name.clone(),
                        e.to_string(),
                    ))
                    .await;
                break;
            }
        }
    });

    let reader_name = config.name.clone();
    let reader_events = events.clone();
    tokio::spawn(async move {
        loop {
            match read_frame(&mut read_half).await {
                Ok(msg) => {
                    if reader_events
                        .send(RuntimeEvent::FederationFrame(
                            reader_name.clone(),
                            Box::new(msg),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    let _ = reader_events
                        .send(RuntimeEvent::FederationDown(
                            reader_name.clone(),
                            e.to_string(),
                        ))
                        .await;
                    break;
                }
            }
        }
    });

    Ok(())
}
