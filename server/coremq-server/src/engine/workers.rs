use std::{net::SocketAddr, sync::Arc};

use axum::{Router, routing::get};
use axum_server::tls_rustls::RustlsConfig;
use tokio::{net::TcpListener, sync::watch, task::JoinHandle};
use tokio_rustls::TlsAcceptor;
use tower_http::cors::CorsLayer;

use crate::{
    engine::Engine,
    enums::protocol::ProtocolType,
    models::listener::ListenerConfig,
    transport::{
        ProtocolState,
        tcp::tcp_connection,
        tls,
        ws::{WsState, ws_handler},
    },
};

impl Engine {
    async fn tcp_worker(
        listener: TcpListener,
        port: u16,
        state: Arc<ProtocolState>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        println!("MQTT TCP listening on port {}", port);

        loop {
            tokio::select! {
                res = listener.accept() => {
                    let (socket, peer) = match res { Ok(v) => v, Err(_) => continue };
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = tcp_connection(socket, state_clone, port, peer).await {
                            println!("TCP connection error: {}", e);
                        }
                    });
                }
                _ = stop_rx.changed() => {
                    println!("Stopping TCP listener on port {}", port);
                    break;
                }
            }
        }
    }

    async fn tls_worker(
        listener: TcpListener,
        acceptor: TlsAcceptor,
        port: u16,
        state: Arc<ProtocolState>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        println!("MQTT TLS listening on port {}", port);

        loop {
            tokio::select! {
                res = listener.accept() => {
                    let (socket, peer) = match res { Ok(v) => v, Err(_) => continue };
                    let acceptor = acceptor.clone();
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        match acceptor.accept(socket).await {
                            Ok(tls_stream) => {
                                if let Err(e) = tcp_connection(tls_stream, state_clone, port, peer).await {
                                    println!("TLS connection error: {}", e);
                                }
                            }
                            Err(e) => println!("TLS handshake error on port {}: {}", port, e),
                        }
                    });
                }
                _ = stop_rx.changed() => {
                    println!("Stopping TLS listener on port {}", port);
                    break;
                }
            }
        }
    }

    async fn ws_worker(
        listener: TcpListener,
        port: u16,
        state: Arc<ProtocolState>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        let ws_state = WsState { engine: state.clone(), port };
        let app = Router::new()
            .route("/mqtt", get(ws_handler))
            .with_state(ws_state)
            .layer(CorsLayer::permissive());

        let server = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>());
        println!("MQTT WS listening on port {}", port);

        tokio::select! {
            res = server => {
                if let Err(e) = res {
                    eprintln!("WS server error on port {}: {}", port, e);
                }
            }
            _ = stop_rx.changed() => {
                println!("Stopping WS listener on port {}", port);
            }
        }
    }

    async fn wss_worker(
        std_listener: std::net::TcpListener,
        config: RustlsConfig,
        port: u16,
        state: Arc<ProtocolState>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        let ws_state = WsState { engine: state.clone(), port };
        let app = Router::new()
            .route("/mqtt", get(ws_handler))
            .with_state(ws_state)
            .layer(CorsLayer::permissive());

        println!("MQTT WSS listening on port {}", port);

        tokio::select! {
            res = axum_server::from_tcp_rustls(std_listener, config)
                .serve(app.into_make_service_with_connect_info::<SocketAddr>()) => {
                if let Err(e) = res {
                    eprintln!("WSS server error on port {}: {}", port, e);
                }
            }
            _ = stop_rx.changed() => {
                println!("Stopping WSS listener on port {}", port);
            }
        }
    }

    /// Start a single listener, returning a runtime error (bind failure, bad TLS cert,
    /// port already in use) instead of panicking. Used at startup and for dynamic creation.
    pub async fn start_one(
        &mut self,
        cfg: &ListenerConfig,
        state: Arc<ProtocolState>,
    ) -> Result<(), String> {
        if self.listeners.contains_key(&cfg.port) {
            return Err(format!("a listener is already running on port {}", cfg.port));
        }

        let host = if cfg.host.is_empty() { "0.0.0.0".to_string() } else { cfg.host.clone() };
        let addr = format!("{}:{}", host, cfg.port);

        let (stop_tx, stop_rx) = watch::channel(false);
        let port = cfg.port;
        let st = state.clone();

        let handle: JoinHandle<()> = match cfg.protocol {
            ProtocolType::Tcp => {
                let listener = TcpListener::bind(&addr).await.map_err(|e| format!("bind {addr}: {e}"))?;
                tokio::spawn(async move { Engine::tcp_worker(listener, port, st, stop_rx).await; })
            }
            ProtocolType::Ws => {
                let listener = TcpListener::bind(&addr).await.map_err(|e| format!("bind {addr}: {e}"))?;
                tokio::spawn(async move { Engine::ws_worker(listener, port, st, stop_rx).await; })
            }
            ProtocolType::Tls => {
                let tls_cfg = cfg.tls.as_ref().ok_or_else(|| "TLS cert/key required for a tls listener".to_string())?;
                let server_config = tls::load_server_config(&tls_cfg.cert, &tls_cfg.key)?;
                let acceptor = TlsAcceptor::from(server_config);
                let listener = TcpListener::bind(&addr).await.map_err(|e| format!("bind {addr}: {e}"))?;
                tokio::spawn(async move { Engine::tls_worker(listener, acceptor, port, st, stop_rx).await; })
            }
            ProtocolType::Wss => {
                let tls_cfg = cfg.tls.as_ref().ok_or_else(|| "TLS cert/key required for a wss listener".to_string())?;
                let config = RustlsConfig::from_pem_file(&tls_cfg.cert, &tls_cfg.key)
                    .await
                    .map_err(|e| format!("load TLS cert/key: {e}"))?;
                let std_listener = std::net::TcpListener::bind(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
                std_listener.set_nonblocking(true).map_err(|e| e.to_string())?;
                tokio::spawn(async move { Engine::wss_worker(std_listener, config, port, st, stop_rx).await; })
            }
        };

        self.listeners.insert(port, (handle, stop_tx, cfg.clone()));
        Ok(())
    }

    pub async fn start_listeners(&mut self, state: Arc<ProtocolState>) {
        // Remember the protocol state so listeners can also be created at runtime.
        self.protocol_state = Some(state.clone());
        let cfgs: Vec<ListenerConfig> = self.config.mqtt.listeners.clone();
        for cfg in cfgs {
            if let Err(e) = self.start_one(&cfg, state.clone()).await {
                eprintln!("Failed to start listener '{}' on port {}: {}", cfg.name, cfg.port, e);
            }
        }
    }

    pub async fn stop_listener(&mut self, port: u16) {
        if let Some((handle, stop_tx, _)) = self.listeners.remove(&port) {
            let _ = stop_tx.send(true);
            // Wait for the worker to finish so its socket is released before any rebind.
            let _ = handle.await;
            println!("Stopped listener on port {}", port);
        }
    }
}
