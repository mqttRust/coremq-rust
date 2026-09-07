mod api;
mod cluster;
mod engine;
mod enums;
mod models;
mod pkg;
mod protocol;
mod services;
mod storage;
mod transport;
mod utils;

use axum::{Router, routing::get};
use std::{net::SocketAddr, sync::{Arc, atomic::AtomicU16}};
use tokio::{net::TcpListener, sync::mpsc};

use crate::{
    api::{api_state::ApiState, router::RouterHandler}, engine::{AdminCommand, ConnectCommand, Engine, EngineChannels, PubSubCommand}, services::{SessionService, jwt::{self, JwtService}, webhook::WebhookDispatcher, auth::AuthService}, storage::redb::{Storage, cluster::ClusterRepo}, transport::{ProtocolState, tcp::tcp_connection, ws::ws_handler}
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pid = std::process::id();
    println!("Current process ID: {}", pid);

    // Install a single, unambiguous rustls crypto provider for all TLS listeners.
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

    let mut config = match utils::config::from_file() {
        Ok(cfg) => cfg,
        Err(e) => panic!("Failed to load config: {}", e)
    };

    let enforcer = match pkg::enforcer::new(config.middleware.clone()).await {
        Ok(enforcer) => enforcer,
        Err(e) => { panic!("Failed to create enforcer: {}", e)}
    };

    let data_dir = std::env::var("COREMQ_DATA")
        .unwrap_or_else(|_| "/etc/coremq/data".to_string());
    let _ = std::fs::create_dir_all(&data_dir);
    let path = format!("{}/coremq.redb", data_dir);
    let db = match pkg::db::new(&path) {
        Ok(db) => db,
        Err(e) => { panic!("Failed to create database: {}", e)}
    };

    let client_service = Arc::new(SessionService::new());
    let jwt_service = Arc::new(JwtService::new(&config.middleware));
    let enforcer = Arc::new(enforcer);
    let db_arc = Arc::new(db);

    /*
      Clustering is opt-in. With no `cluster:` section the broker builds exactly
      as it did before: no identity, no replication hook, no peer port.
    */
    let cluster_config = config.cluster.clone().filter(|c| c.enabled);
    let cluster_repo = ClusterRepo::new(db_arc.clone());

    let (identity, replication, meta_rx) = match &cluster_config {
        Some(cfg) => {
            let identity = cluster::resolve_identity(cfg, &cluster_repo, None)
                .map_err(|e| anyhow::anyhow!("cluster identity: {}", e))?;
            let (replication, meta_rx) =
                cluster::build_replication(identity.id.clone(), cluster_repo.clone());
            (Some(identity), Some(replication), Some(meta_rx))
        }
        None => (None, None, None),
    };

    let storage = Arc::new(Storage::with_replication(db_arc, replication));
    let webhook = WebhookDispatcher::new(storage.clone());
    let auth = AuthService::new(storage.clone());

    // Merge dynamically-created (persisted) listeners; config-defined listeners win on port.
    if let Ok(persisted) = storage.listener.get_all() {
        for cfg in persisted {
            if !config.mqtt.listeners.iter().any(|l| l.port == cfg.port) {
                config.mqtt.listeners.push(cfg);
            }
        }
    }

    let (connect_tx, connect_rx) = mpsc::unbounded_channel::<ConnectCommand>();
    let (pubsub_tx, pubsub_rx) = mpsc::unbounded_channel::<PubSubCommand>();
    let (admin_tx, admin_rx) = mpsc::unbounded_channel::<AdminCommand>();

    let channels = EngineChannels {
        connect_rx,
        pubsub_rx,
        admin_rx,
    };

    let engine_channels = Arc::new(ProtocolState {
        connect_tx: connect_tx.clone(),
        pubsub_tx: pubsub_tx.clone(),
        auth: auth.clone(),
    });

    /*
      Start the cluster before the engine so the engine can be handed a handle.
      A failure here is logged and clustering stays off rather than taking the
      whole broker down.
    */
    let cluster_handle = match (cluster_config, identity, meta_rx) {
        (Some(cfg), Some(self_desc), Some(meta_rx)) => {
            match cluster::discovery::build(&cfg.discovery, self_desc.advertise_addr) {
                Ok(discovery) => {
                    let deps = cluster::runtime::RuntimeDeps {
                        config: cfg,
                        federation: config.federation.clone(),
                        self_desc,
                        discovery,
                        repo: cluster_repo,
                        sessions: client_service.clone(),
                        pubsub_tx: pubsub_tx.clone(),
                        connect_tx: connect_tx.clone(),
                        auth: auth.clone(),
                        meta_rx,
                    };
                    match cluster::runtime::spawn(deps) {
                        Ok(handle) => Some(handle),
                        Err(e) => {
                            eprintln!("Cluster failed to start, continuing standalone: {}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Cluster discovery misconfigured, continuing standalone: {}", e);
                    None
                }
            }
        }
        _ => None,
    };

    let mut engine = Engine::new(client_service.clone(), config,  channels, webhook.clone())
        .with_cluster(cluster_handle.clone());
    engine.start_listeners(engine_channels.clone()).await;

    tokio::spawn(async move {
        engine.run().await;
    });

    let state = ApiState {
        jwt_service: jwt_service.clone(),
        enforcer: enforcer.clone(),
        engine: admin_tx.clone(),
        storage: storage.clone(),
        packet_id_counter: Arc::new(AtomicU16::new(1)),
        webhook: webhook.clone(),
        auth: auth.clone(),
        cluster: cluster_handle,
    };

    let router = RouterHandler::new();
    let addr = format!("{}:{}", "0.0.0.0", 18083);
    let listener = TcpListener::bind(addr.clone()).await?;
    println!("Admin Panel running on {}", addr);

    axum::serve(listener, router.create_router(state))
        .await
        .unwrap();

    Ok(())
}