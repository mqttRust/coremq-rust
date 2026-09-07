use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

use crate::cluster::discovery::Discovery;
use crate::cluster::federation::{Federation, LinkState};
use crate::cluster::handle::{
    ClusterHandle, ClusterRequest, ClusterStatus, FederationView, NodeView, Origin, RemoteSession,
    RouteView,
};
use crate::cluster::membership::{MemberState, Membership, Transition};
use crate::cluster::meta::{MetaApplier, skew_warning};
use crate::cluster::node::{LeaveReason, NodeDescriptor, NodeId};
use crate::cluster::peer::{self, Direction, PeerEvent, PeerHandle};
use crate::cluster::protocol::{ClusterMessage, MetaEntry, WirePublish};
use crate::cluster::router::RouteTable;
use crate::engine::{ConnectCommand, PubSubCommand};
use crate::models::config::cluster::{ClusterConfig, FederationConfig};
use crate::protocol::packets::PublishPacket;
use crate::services::SessionService;
use crate::storage::redb::cluster::ClusterRepo;

/* Peers may disagree with our clock by this much before we complain. */
const CLOCK_SKEW_WARN_MS: i64 = 5_000;

/* How long a scatter-gather session query waits for peers before answering. */
const SESSION_QUERY_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ClusterRuntime {
    self_desc: NodeDescriptor,
    config: ClusterConfig,

    membership: Membership,
    routes: RouteTable,
    federation: Federation,

    peers: HashMap<NodeId, PeerHandle>,
    /* Addresses with a dial already in flight, so we do not stampede a peer. */
    dialing: HashSet<SocketAddr>,

    /* Mirror of the configured links, read by the accept loop. */
    fed_registry: peer::FederationRegistry,

    discovery: Arc<dyn Discovery>,
    applier: MetaApplier,
    sessions: Arc<SessionService>,

    /* Our own route epoch, bumped on every announced change. */
    epoch: u64,
    local_filters: HashSet<String>,

    probe_seq: u64,
    /* Round-robin cursor for choosing indirect probers. */
    prober_cursor: usize,

    /* Packet ids for forwarded QoS > 0 publishes; encode_publish panics on None. */
    packet_ids: AtomicU16,

    pending_sessions: HashMap<u64, PendingSessionQuery>,
    next_query_id: u64,

    events_tx: mpsc::Sender<RuntimeEvent>,
    pubsub_tx: mpsc::UnboundedSender<PubSubCommand>,
    connect_tx: mpsc::UnboundedSender<ConnectCommand>,
    auth: Arc<crate::services::auth::AuthService>,
}

struct PendingSessionQuery {
    reply: oneshot::Sender<Vec<RemoteSession>>,
    collected: Vec<RemoteSession>,
    awaiting: HashSet<NodeId>,
    deadline: Instant,
}

/*
  Everything that can wake the runtime. Peer I/O, discovery and dialling all run
  in their own tasks and funnel into this one channel, so the runtime itself
  never awaits anything but the next event.
*/
pub enum RuntimeEvent {
    Peer(PeerEvent),
    Established(Box<peer::HandshakeResult>),
    DialFailed(SocketAddr),
    Candidates(Vec<SocketAddr>),
    MetaLocal(MetaEntry),
    FederationUp(String, mpsc::Sender<ClusterMessage>, Vec<String>),
    FederationDown(String, String),
    FederationFrame(String, Box<ClusterMessage>),
    FederationInbound(Box<peer::FederationInbound>),
}

pub struct RuntimeDeps {
    pub config: ClusterConfig,
    pub federation: Vec<FederationConfig>,
    pub self_desc: NodeDescriptor,
    pub discovery: Arc<dyn Discovery>,
    pub repo: ClusterRepo,
    pub sessions: Arc<SessionService>,
    pub pubsub_tx: mpsc::UnboundedSender<PubSubCommand>,
    pub connect_tx: mpsc::UnboundedSender<ConnectCommand>,
    pub auth: Arc<crate::services::auth::AuthService>,
    pub meta_rx: mpsc::Receiver<MetaEntry>,
}

/*
  Start the cluster subsystem and return a handle for the engine and admin API.
*/
pub fn spawn(deps: RuntimeDeps) -> anyhow::Result<ClusterHandle> {
    let (req_tx, req_rx) = mpsc::channel::<ClusterRequest>(crate::cluster::handle::CLUSTER_QUEUE_DEPTH);
    let (events_tx, events_rx) = mpsc::channel::<RuntimeEvent>(crate::cluster::handle::CLUSTER_QUEUE_DEPTH);

    let handle = ClusterHandle::new(
        req_tx,
        deps.self_desc.id.clone(),
        deps.self_desc.cluster.clone(),
    );

    let bind: SocketAddr = deps
        .config
        .bind
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid cluster.bind '{}'", deps.config.bind))?;

    let membership = Membership::new(deps.self_desc.id.clone(), deps.config.failure_detector.clone());
    let federation = Federation::new(deps.self_desc.cluster.clone(), &deps.federation);
    let applier = MetaApplier::new(deps.repo);

    /*
      Shared with the accept loop so an inbound federation link can be checked
      against its own secret, and so links added through the API are visible
      without restarting the listener.
    */
    let fed_registry: peer::FederationRegistry = Arc::new(tokio::sync::RwLock::new(
        deps.federation
            .iter()
            .map(|c| (c.name.clone(), c.clone()))
            .collect(),
    ));

    let runtime = ClusterRuntime {
        self_desc: deps.self_desc.clone(),
        config: deps.config.clone(),
        membership,
        routes: RouteTable::new(),
        federation,
        peers: HashMap::new(),
        dialing: HashSet::new(),
        fed_registry: fed_registry.clone(),
        discovery: deps.discovery.clone(),
        applier,
        sessions: deps.sessions,
        epoch: 0,
        local_filters: HashSet::new(),
        probe_seq: 0,
        prober_cursor: 0,
        packet_ids: AtomicU16::new(1),
        pending_sessions: HashMap::new(),
        next_query_id: 1,
        events_tx: events_tx.clone(),
        pubsub_tx: deps.pubsub_tx,
        connect_tx: deps.connect_tx,
        auth: deps.auth,
    };

    /* Accept loop for peers and remote clusters dialling us. */
    let accept_events = events_tx.clone();
    let accept_desc = deps.self_desc.clone();
    let accept_secret = deps.config.secret.clone();
    let accept_fed = fed_registry.clone();
    tokio::spawn(async move {
        match TcpListener::bind(bind).await {
            Ok(listener) => {
                println!("cluster: peer listener on {}", bind);
                accept_loop(listener, accept_desc, accept_secret, accept_fed, accept_events).await;
            }
            Err(e) => println!("cluster: cannot bind peer listener on {}: {}", bind, e),
        }
    });

    /* Discovery sweeps. */
    let disc_events = events_tx.clone();
    let disc = deps.discovery.clone();
    let disc_interval = deps.config.discovery.interval;
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(disc_interval);
        loop {
            ticker.tick().await;
            match disc.discover().await {
                Ok(addrs) => {
                    if disc_events.send(RuntimeEvent::Candidates(addrs)).await.is_err() {
                        break;
                    }
                }
                Err(e) => println!("cluster: {} discovery failed: {}", disc.name(), e),
            }
        }
    });

    /* Local metadata writes waiting to be gossiped. */
    let meta_events = events_tx.clone();
    let mut meta_rx = deps.meta_rx;
    tokio::spawn(async move {
        while let Some(entry) = meta_rx.recv().await {
            if meta_events.send(RuntimeEvent::MetaLocal(entry)).await.is_err() {
                break;
            }
        }
    });

    tokio::spawn(async move {
        runtime.run(req_rx, events_rx).await;
    });

    Ok(handle)
}

async fn accept_loop(
    listener: TcpListener,
    self_desc: NodeDescriptor,
    secret: String,
    federation: peer::FederationRegistry,
    events: mpsc::Sender<RuntimeEvent>,
) {
    loop {
        let Ok((stream, addr)) = listener.accept().await else {
            continue;
        };

        let desc = self_desc.clone();
        let secret = secret.clone();
        let federation = federation.clone();
        let events = events.clone();

        tokio::spawn(async move {
            match peer::accept(stream, addr, &desc, &secret, &federation).await {
                Ok(peer::Accepted::Peer(result)) => {
                    let _ = events.send(RuntimeEvent::Established(result)).await;
                }
                Ok(peer::Accepted::Federation(inbound)) => {
                    let _ = events.send(RuntimeEvent::FederationInbound(inbound)).await;
                }
                Ok(peer::Accepted::Rejected) => {}
                Err(e) => println!("cluster: handshake with {} failed: {}", addr, e),
            }
        });
    }
}

impl ClusterRuntime {
    async fn run(
        mut self,
        mut requests: mpsc::Receiver<ClusterRequest>,
        mut events: mpsc::Receiver<RuntimeEvent>,
    ) {
        let mut probe = tokio::time::interval(self.config.failure_detector.probe_interval);
        let mut reconcile = tokio::time::interval(self.config.cleanup.reconcile_interval);
        let mut orphan_sweep = tokio::time::interval(self.config.cleanup.orphan_sweep_interval);
        let mut tombstone_sweep = tokio::time::interval(Duration::from_secs(600));
        let mut federation_tick = tokio::time::interval(Duration::from_secs(5));

        probe.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        reconcile.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        println!(
            "cluster: node {} joined cluster '{}' (incarnation {}, discovery {})",
            self.self_desc.id, self.self_desc.cluster, self.self_desc.incarnation, self.discovery.name()
        );

        loop {
            tokio::select! {
                Some(req) = requests.recv() => {
                    if self.handle_request(req).await {
                        break;
                    }
                }
                Some(event) = events.recv() => {
                    self.handle_event(event).await;
                }
                _ = probe.tick() => {
                    self.on_probe().await;
                    self.expire_session_queries();
                }
                _ = reconcile.tick() => {
                    self.on_reconcile();
                }
                _ = orphan_sweep.tick() => {
                    self.on_orphan_sweep();
                }
                _ = tombstone_sweep.tick() => {
                    self.on_tombstone_sweep();
                }
                _ = federation_tick.tick() => {
                    self.on_federation_tick();
                }
            }
        }

        self.broadcast_leave();
    }

    /* Returns true when the runtime should stop. */
    async fn handle_request(&mut self, req: ClusterRequest) -> bool {
        match req {
            ClusterRequest::RouteAdd(filter) => {
                if self.local_filters.insert(filter.clone()) {
                    self.epoch += 1;
                    let msg = ClusterMessage::RouteAdd {
                        node: self.self_desc.id.clone(),
                        filter,
                        epoch: self.epoch,
                    };
                    self.broadcast(msg);
                }
            }

            ClusterRequest::RouteDel(filter) => {
                if self.local_filters.remove(&filter) {
                    self.epoch += 1;
                    let msg = ClusterMessage::RouteDel {
                        node: self.self_desc.id.clone(),
                        filter,
                        epoch: self.epoch,
                    };
                    self.broadcast(msg);
                }
            }

            ClusterRequest::Forward(packet) => {
                self.forward_publish(&packet);
            }

            ClusterRequest::ClaimSession(client_id) => {
                let msg = ClusterMessage::SessionClaim {
                    client_id,
                    node: self.self_desc.id.clone(),
                    claimed_at: crate::cluster::meta::now_ms(),
                };
                self.broadcast(msg);
            }

            ClusterRequest::Status(reply) => {
                let _ = reply.send(self.status());
            }

            ClusterRequest::Nodes(reply) => {
                let _ = reply.send(self.nodes());
            }

            ClusterRequest::Routes(reply) => {
                let views = self
                    .routes
                    .entries()
                    .into_iter()
                    .map(|(filter, node)| RouteView {
                        filter,
                        node: node.to_string(),
                    })
                    .chain(self.local_filters.iter().map(|f| RouteView {
                        filter: f.clone(),
                        node: self.self_desc.id.to_string(),
                    }))
                    .collect();
                let _ = reply.send(views);
            }

            ClusterRequest::Sessions(reply) => {
                self.start_session_query(reply);
            }

            ClusterRequest::Join(addr, reply) => {
                if self.peers.values().any(|p| p.addr == addr) {
                    let _ = reply.send(Err("already connected to that address".to_string()));
                } else {
                    self.dial(addr);
                    let _ = reply.send(Ok(()));
                }
            }

            ClusterRequest::Evict(node, reply) => {
                let existed = self.membership.forget(&node);
                self.routes.purge_node(&node);
                if let Some(mut handle) = self.peers.remove(&node) {
                    handle.try_send(ClusterMessage::Leave {
                        node: self.self_desc.id.clone(),
                        reason: LeaveReason::Evicted,
                    });
                }
                let _ = reply.send(existed);
            }

            ClusterRequest::FederationStatus(reply) => {
                let views = self
                    .federation
                    .all()
                    .map(|l| FederationView {
                        name: l.config.name.clone(),
                        endpoints: l.config.endpoints.clone(),
                        forward: l.config.forward.clone(),
                        accept: l.config.accept.clone(),
                        state: l.state,
                        sent: l.sent,
                        received: l.received,
                        dropped: l.dropped,
                        last_error: l.last_error.clone(),
                    })
                    .collect();
                let _ = reply.send(views);
            }

            ClusterRequest::FederationAdd(config, reply) => {
                let name = config.name.clone();
                if name.trim().is_empty() {
                    let _ = reply.send(Err("federation link needs a name".to_string()));
                } else {
                    let cloned = (*config).clone();
                    if !self.federation.add(*config) {
                        let _ = reply.send(Err(format!("federation link '{}' already exists", name)));
                    } else {
                        /* The accept loop must see the new link before it can be dialled to us. */
                        self.fed_registry.write().await.insert(name, cloned);
                        self.on_federation_tick();
                        let _ = reply.send(Ok(()));
                    }
                }
            }

            ClusterRequest::FederationRemove(name, reply) => {
                let removed = self.federation.remove(&name).is_some();
                if removed {
                    self.fed_registry.write().await.remove(&name);
                }
                let _ = reply.send(removed);
            }

            ClusterRequest::Shutdown(reply) => {
                let _ = reply.send(());
                return true;
            }
        }

        false
    }

    async fn handle_event(&mut self, event: RuntimeEvent) {
        match event {
            RuntimeEvent::Established(result) => self.on_established(*result).await,
            RuntimeEvent::DialFailed(addr) => {
                self.dialing.remove(&addr);
            }
            RuntimeEvent::Peer(PeerEvent::Frame(from, msg)) => self.on_frame(from, msg).await,
            RuntimeEvent::Peer(PeerEvent::Disconnected(id, direction)) => {
                self.on_peer_disconnected(id, direction);
            }
            RuntimeEvent::Candidates(addrs) => self.on_candidates(addrs),
            RuntimeEvent::MetaLocal(entry) => {
                self.broadcast(ClusterMessage::MetaDelta {
                    entries: vec![entry],
                });
            }
            RuntimeEvent::FederationUp(name, tx, remote_accept) => {
                if let Some(link) = self.federation.get_mut(&name) {
                    link.set_state(LinkState::Up);
                    link.tx = Some(tx);
                    link.remote_accept = remote_accept;
                    link.last_error = None;
                    println!("cluster: federation link '{}' is up", name);
                }
            }
            RuntimeEvent::FederationDown(name, reason) => {
                if let Some(link) = self.federation.get_mut(&name) {
                    link.set_state(LinkState::Down);
                    link.tx = None;
                    link.remote_accept.clear();
                    link.last_error = Some(reason.clone());
                }
                println!("cluster: federation link '{}' is down: {}", name, reason);
            }
            RuntimeEvent::FederationFrame(name, msg) => self.on_federation_frame(name, *msg),
            RuntimeEvent::FederationInbound(inbound) => self.on_federation_inbound(*inbound),
        }
    }

    /*
      A remote cluster dialled us. Only the elected owner in *their* cluster
      dials, so accepting here does not duplicate anything.
    */
    fn on_federation_inbound(&mut self, inbound: peer::FederationInbound) {
        let name = inbound.link.clone();

        let Some(link) = self.federation.get_mut(&name) else {
            println!(
                "cluster: dropping inbound federation link '{}'; it is no longer configured",
                name
            );
            return;
        };

        link.set_state(LinkState::Up);
        link.tx = Some(inbound.tx);
        link.remote_accept = inbound.remote_accept;
        link.last_error = None;

        println!(
            "cluster: federation link '{}' accepted from cluster '{}'",
            name, inbound.remote_cluster
        );

        let events = self.events_tx.clone();
        let mut stream = inbound.stream;
        tokio::spawn(async move {
            loop {
                match crate::cluster::protocol::read_frame(&mut stream).await {
                    Ok(msg) => {
                        if events
                            .send(RuntimeEvent::FederationFrame(name.clone(), Box::new(msg)))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = events
                            .send(RuntimeEvent::FederationDown(name.clone(), e.to_string()))
                            .await;
                        break;
                    }
                }
            }
        });
    }

    async fn on_established(&mut self, result: peer::HandshakeResult) {
        let established = result.established;
        let peer_id = established.desc.id.clone();

        self.dialing.remove(&established.addr);

        if let Some(warning) = skew_warning(established.desc.started_at, CLOCK_SKEW_WARN_MS) {
            println!("cluster: {} — {}", peer_id, warning);
        }

        /*
          Simultaneous dial leaves two connections. Both sides independently keep
          the one dialled by the smaller node id, so they converge on the same link.
        */
        if let Some(existing) = self.peers.get(&peer_id) {
            let preferred = PeerHandle::preferred_direction(&self.self_desc.id, &peer_id);
            if existing.direction == preferred || established.direction != preferred {
                return;
            }
        }

        let handle = PeerHandle {
            desc: established.desc.clone(),
            direction: established.direction,
            tx: established.tx,
            addr: established.addr,
            dropped: 0,
        };

        self.peers.insert(peer_id.clone(), handle);

        let events = self.events_tx.clone();
        let (peer_events_tx, mut peer_events_rx) = mpsc::channel::<PeerEvent>(256);
        tokio::spawn(async move {
            peer::reader_loop(result.reader, peer_events_tx).await;
        });
        tokio::spawn(async move {
            while let Some(e) = peer_events_rx.recv().await {
                if events.send(RuntimeEvent::Peer(e)).await.is_err() {
                    break;
                }
            }
        });

        if let Some(transition) = self.membership.observe(established.desc) {
            self.apply_transition(transition);
        }

        /*
          Announce ourselves, then ask for anything we missed. A reconnecting
          peer's epoch tells it whether a delta stream or a snapshot is needed.
        */
        let entries = self.membership.to_entries(&self.self_desc);
        self.send_to(&peer_id, ClusterMessage::Membership { entries });

        let snapshot = ClusterMessage::RouteSnapshot {
            node: self.self_desc.id.clone(),
            filters: self.local_filters.iter().cloned().collect(),
            epoch: self.epoch,
        };
        self.send_to(&peer_id, snapshot);

        let since = self.routes.epoch_for(&peer_id);
        self.send_to(
            &peer_id,
            ClusterMessage::RouteSyncRequest {
                node: self.self_desc.id.clone(),
                since_epoch: since,
            },
        );

        self.send_to(&peer_id, ClusterMessage::MetaSyncRequest);

        println!("cluster: peer {} connected ({})", peer_id, established.addr);
    }

    async fn on_frame(&mut self, from: NodeId, msg: ClusterMessage) {
        /* Any frame is proof of life. */
        if let Some(transition) = self.membership.touch(&from) {
            self.apply_transition(transition);
        }

        match msg {
            ClusterMessage::Ping { seq } => {
                self.send_to(&from, ClusterMessage::Pong { seq });
            }
            ClusterMessage::Pong { .. } => {}

            ClusterMessage::PingReq { target, seq } => {
                /*
                  A peer cannot reach `target` and is asking us. We answer from
                  our own membership view rather than probing synchronously.
                */
                let reachable = self
                    .membership
                    .get(&target)
                    .map(|m| m.state == MemberState::Alive)
                    .unwrap_or(false);

                self.send_to(
                    &from,
                    ClusterMessage::PingAck {
                        target,
                        seq,
                        reachable,
                    },
                );
            }

            ClusterMessage::PingAck { target, reachable, .. } => {
                if reachable {
                    if let Some(transition) = self.membership.touch(&target) {
                        self.apply_transition(transition);
                    }
                }
            }

            ClusterMessage::Membership { entries } => {
                let transitions = self.membership.merge(entries);
                for t in transitions {
                    self.apply_transition(t);
                }
            }

            ClusterMessage::RouteAdd { node, filter, epoch } => {
                self.routes.add(&node, &filter);
                self.routes.set_epoch(&node, epoch);
            }

            ClusterMessage::RouteDel { node, filter, epoch } => {
                self.routes.remove(&node, &filter);
                self.routes.set_epoch(&node, epoch);
            }

            ClusterMessage::RouteSyncRequest { node, .. } => {
                /*
                  Always answer with a full snapshot. Retaining a delta log to
                  serve partial syncs would cost more than the snapshot saves at
                  the filter counts this targets.
                */
                let snapshot = ClusterMessage::RouteSnapshot {
                    node: self.self_desc.id.clone(),
                    filters: self.local_filters.iter().cloned().collect(),
                    epoch: self.epoch,
                };
                let _ = node;
                self.send_to(&from, snapshot);
            }

            ClusterMessage::RouteSnapshot { node, filters, epoch } => {
                /* The owner is authoritative: anything it no longer claims is stale. */
                self.routes.replace_node(&node, &filters);
                self.routes.set_epoch(&node, epoch);
            }

            ClusterMessage::Forward {
                origin,
                packet,
                cluster_path,
            } => {
                self.deliver_remote(packet, Origin::Remote(origin), cluster_path);
            }

            ClusterMessage::SessionClaim { client_id, node, .. } => {
                /*
                  MQTT requires a client id to be unique cluster-wide. Another
                  node took this one, so our copy must go.
                */
                if node != self.self_desc.id && self.sessions.get_session(&client_id).is_some() {
                    println!(
                        "cluster: client '{}' was claimed by {}, dropping our session",
                        client_id, node
                    );
                    let _ = self.connect_tx.send(ConnectCommand::Takeover(client_id));
                }
            }

            ClusterMessage::MetaDelta { entries } => {
                let outcome = self.applier.apply(entries);
                if outcome.auth_changed {
                    self.auth.reload();
                }
            }

            ClusterMessage::MetaSyncRequest => match self.applier.snapshot() {
                Ok(entries) if !entries.is_empty() => {
                    self.send_to(&from, ClusterMessage::MetaDelta { entries });
                }
                Ok(_) => {}
                Err(e) => println!("cluster: cannot build metadata snapshot: {}", e),
            },

            ClusterMessage::Leave { node, reason } => {
                println!("cluster: node {} left ({})", node, reason);
                if let Some(transition) = self.membership.mark_left(&node) {
                    self.apply_transition(transition);
                }
            }

            ClusterMessage::Hello { .. }
            | ClusterMessage::HelloAck { .. }
            | ClusterMessage::FederationHello { .. }
            | ClusterMessage::FederationAck { .. } => {
                /* Handshake frames are consumed before the reader loop starts. */
            }
        }
    }

    fn on_peer_disconnected(&mut self, id: NodeId, direction: Direction) {
        /*
          Only forget the peer if this is the connection we were actually using.
          A losing duplicate closing must not tear down the live link.
        */
        if let Some(existing) = self.peers.get(&id) {
            if existing.direction != direction {
                return;
            }
        }

        self.peers.remove(&id);

        /*
          A dropped connection is NOT a purge. It marks the node quiet and lets
          the failure detector decide; tearing down routes here would flap the
          table on every transient hiccup.
        */
        println!("cluster: peer {} disconnected", id);
    }

    fn on_candidates(&mut self, addrs: Vec<SocketAddr>) {
        let connected: HashSet<SocketAddr> = self.peers.values().map(|p| p.addr).collect();
        let advertised: HashSet<SocketAddr> =
            self.peers.values().map(|p| p.desc.advertise_addr).collect();

        for addr in addrs {
            if addr == self.self_desc.advertise_addr
                || connected.contains(&addr)
                || advertised.contains(&addr)
                || self.dialing.contains(&addr)
            {
                continue;
            }
            self.dial(addr);
        }
    }

    fn dial(&mut self, addr: SocketAddr) {
        self.dialing.insert(addr);

        let desc = self.self_desc.clone();
        let secret = self.config.secret.clone();
        let events = self.events_tx.clone();

        tokio::spawn(async move {
            match peer::dial(addr, &desc, &secret).await {
                Ok(Some(result)) => {
                    let _ = events.send(RuntimeEvent::Established(Box::new(result))).await;
                }
                Ok(None) => {
                    let _ = events.send(RuntimeEvent::DialFailed(addr)).await;
                }
                Err(_) => {
                    let _ = events.send(RuntimeEvent::DialFailed(addr)).await;
                }
            }
        });
    }

    async fn on_probe(&mut self) {
        self.probe_seq += 1;
        let seq = self.probe_seq;

        let peer_ids: Vec<NodeId> = self.peers.keys().cloned().collect();
        for id in &peer_ids {
            self.send_to(id, ClusterMessage::Ping { seq });
        }

        /*
          Ask other peers about anyone going quiet before suspecting them. One
          bad link between two nodes must not evict a healthy third.
        */
        if self.config.failure_detector.indirect_probes > 0 {
            let quiet = self.membership.needs_indirect_probe();
            for target in quiet {
                let helpers = self.pick_probers(&target);
                for helper in helpers {
                    self.send_to(
                        &helper,
                        ClusterMessage::PingReq {
                            target: target.clone(),
                            seq,
                        },
                    );
                }
            }
        }

        for transition in self.membership.tick() {
            self.apply_transition(transition);
        }
    }

    fn pick_probers(&mut self, target: &NodeId) -> Vec<NodeId> {
        let candidates: Vec<NodeId> = self
            .peers
            .keys()
            .filter(|id| *id != target)
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Vec::new();
        }

        let k = self.config.failure_detector.indirect_probes.min(candidates.len());
        let mut out = Vec::with_capacity(k);
        for i in 0..k {
            out.push(candidates[(self.prober_cursor + i) % candidates.len()].clone());
        }
        self.prober_cursor = self.prober_cursor.wrapping_add(1);
        out
    }

    /*
      The cleanup matrix. Only Died and Restarted purge; Suspected deliberately
      does nothing, and Recovered only re-syncs.
    */
    fn apply_transition(&mut self, transition: Transition) {
        match transition {
            Transition::Joined(id) => {
                println!("cluster: node {} joined", id);
            }

            Transition::Suspected(id) => {
                /* Not actionable on purpose. Routes stay, forwarding continues. */
                println!("cluster: node {} is unresponsive (suspect)", id);
            }

            Transition::Died(id) => {
                let purged = self.routes.purge_node(&id);
                self.membership.mark_purged(&id);
                self.peers.remove(&id);
                self.pending_sessions
                    .values_mut()
                    .for_each(|q| { q.awaiting.remove(&id); });

                println!(
                    "cluster: node {} is dead, purged {} route(s)",
                    id,
                    purged.len()
                );
            }

            Transition::Restarted(id) => {
                /* Every route we hold for the old incarnation is stale. */
                let purged = self.routes.purge_node(&id);
                println!(
                    "cluster: node {} restarted, dropped {} stale route(s)",
                    id,
                    purged.len()
                );
                self.send_to(
                    &id,
                    ClusterMessage::RouteSyncRequest {
                        node: self.self_desc.id.clone(),
                        since_epoch: 0,
                    },
                );
            }

            Transition::Recovered(id) => {
                println!("cluster: node {} is responsive again", id);
                self.send_to(
                    &id,
                    ClusterMessage::RouteSyncRequest {
                        node: self.self_desc.id.clone(),
                        since_epoch: self.routes.epoch_for(&id),
                    },
                );
            }
        }
    }

    fn on_reconcile(&mut self) {
        let snapshot = ClusterMessage::RouteSnapshot {
            node: self.self_desc.id.clone(),
            filters: self.local_filters.iter().cloned().collect(),
            epoch: self.epoch,
        };
        self.broadcast(snapshot);

        let entries = self.membership.to_entries(&self.self_desc);
        self.broadcast(ClusterMessage::Membership { entries });
    }

    fn on_orphan_sweep(&mut self) {
        let live = self.membership.live_ids();
        let orphans = self.routes.sweep_orphans(&live);

        if !orphans.is_empty() {
            println!(
                "cluster: orphan sweep dropped routes for {} unknown node(s)",
                orphans.len()
            );
        }

        /* Forget nodes that have been dead long enough to be purged. */
        let stale: Vec<NodeId> = self
            .membership
            .all()
            .filter(|m| matches!(m.state, MemberState::Dead | MemberState::Left) && m.purged)
            .filter(|m| m.state_since.elapsed() > self.config.cleanup.orphan_sweep_interval * 2)
            .map(|m| m.desc.id.clone())
            .collect();

        for id in stale {
            self.membership.forget(&id);
        }
    }

    fn on_tombstone_sweep(&mut self) {
        match self.applier.sweep_tombstones(self.config.cleanup.tombstone_ttl) {
            Ok(0) => {}
            Ok(n) => println!("cluster: expired {} tombstone(s)", n),
            Err(e) => println!("cluster: tombstone sweep failed: {}", e),
        }
    }

    fn on_federation_tick(&mut self) {
        let live = self.membership.alive_ids();
        let is_owner = Federation::is_owner(&self.self_desc.id, &live);

        let names = self.federation.names();
        for name in names {
            let Some(link) = self.federation.get_mut(&name) else {
                continue;
            };

            if !is_owner {
                /*
                  Another node owns the links. Standing by avoids duplicating
                  every federated publish once per node in this cluster.
                */
                if link.state != LinkState::Standby {
                    link.set_state(LinkState::Standby);
                    link.tx = None;
                }
                continue;
            }

            if matches!(link.state, LinkState::Up | LinkState::Connecting) {
                continue;
            }

            link.set_state(LinkState::Connecting);
            let config = link.config.clone();
            let cluster = self.self_desc.cluster.clone();
            let events = self.events_tx.clone();

            tokio::spawn(async move {
                crate::cluster::federation_link::connect(config, cluster, events).await;
            });
        }
    }

    /*
      Fan a locally-originated publish out to every interested node, exactly once
      per node, then offer it to the federation links.
    */
    fn forward_publish(&mut self, packet: &PublishPacket) {
        let targets = self.routes.match_nodes(&packet.topic);

        if !targets.is_empty() {
            let wire = WirePublish::from(packet);
            for node in targets {
                if node == self.self_desc.id {
                    continue;
                }
                self.send_to(
                    &node,
                    ClusterMessage::Forward {
                        origin: self.self_desc.id.clone(),
                        packet: wire.clone(),
                        cluster_path: vec![self.self_desc.cluster.clone()],
                    },
                );
            }
        }

        self.federate(packet, &[self.self_desc.cluster.clone()]);
    }

    fn federate(&mut self, packet: &PublishPacket, path: &[String]) {
        let names = self.federation.names();
        if names.is_empty() {
            return;
        }

        let wire = WirePublish::from(packet);
        let origin = self.self_desc.id.clone();

        for name in names {
            let Some(link) = self.federation.get_mut(&name) else {
                continue;
            };
            if link.state != LinkState::Up || !link.should_forward(&packet.topic) {
                continue;
            }

            let msg = ClusterMessage::Forward {
                origin: origin.clone(),
                packet: wire.clone(),
                cluster_path: path.to_vec(),
            };

            match link.tx.as_ref().map(|tx| tx.try_send(msg)) {
                Some(Ok(())) => link.sent += 1,
                _ => link.dropped += 1,
            }
        }
    }

    /*
      Deliver a message that arrived from a peer or a federated cluster.

      It is delivered locally and goes no further: the origin node already fanned
      it out to every node in its cluster, so re-forwarding would loop.
    */
    fn deliver_remote(&mut self, wire: WirePublish, origin: Origin, cluster_path: Vec<String>) {
        if self.federation.would_loop(&cluster_path) && !matches!(origin, Origin::Remote(_)) {
            return;
        }

        let packet_id = if wire.qos > 0 {
            Some(self.next_packet_id())
        } else {
            None
        };

        let packet = wire.into_packet(packet_id);
        let _ = self.pubsub_tx.send(PubSubCommand::Publish(packet, origin));
    }

    fn on_federation_frame(&mut self, link_name: String, msg: ClusterMessage) {
        let ClusterMessage::Forward {
            packet,
            mut cluster_path,
            ..
        } = msg
        else {
            return;
        };

        let Some(link) = self.federation.get_mut(&link_name) else {
            return;
        };

        if !link.should_accept(&packet.topic) {
            link.dropped += 1;
            return;
        }

        link.received += 1;

        if self.federation.would_loop(&cluster_path) {
            return;
        }

        cluster_path.push(self.self_desc.cluster.clone());

        /*
          A federated message enters this cluster at one node, so it still has to
          reach the other nodes here — hence a local fan-out with a path that now
          contains us, which stops it bouncing back.
        */
        let targets = self.routes.match_nodes(&packet.topic);
        for node in targets {
            if node == self.self_desc.id {
                continue;
            }
            self.send_to(
                &node,
                ClusterMessage::Forward {
                    origin: self.self_desc.id.clone(),
                    packet: packet.clone(),
                    cluster_path: cluster_path.clone(),
                },
            );
        }

        self.deliver_remote(packet, Origin::Federation(link_name), cluster_path);
    }

    fn next_packet_id(&self) -> u16 {
        loop {
            let id = self.packet_ids.fetch_add(1, Ordering::Relaxed);
            if id != 0 {
                return id;
            }
        }
    }

    fn start_session_query(&mut self, reply: oneshot::Sender<Vec<RemoteSession>>) {
        let mut collected: Vec<RemoteSession> = self
            .sessions
            .all_sessions()
            .into_iter()
            .map(|s| RemoteSession {
                client_id: s.client_id,
                username: s.username,
                node: self.self_desc.id.to_string(),
                remote_addr: s.remote_addr.to_string(),
                connected_port: s.connected_port,
                connected_at: s.connected_at.to_rfc3339(),
                subscriptions: s.subscriptions.len(),
            })
            .collect();

        let awaiting: HashSet<NodeId> = self.peers.keys().cloned().collect();

        /*
          No peers means the local view is the whole answer; replying now avoids
          making a single-node cluster wait for a timeout.
        */
        if awaiting.is_empty() {
            collected.sort_by(|a, b| a.client_id.cmp(&b.client_id));
            let _ = reply.send(collected);
            return;
        }

        let id = self.next_query_id;
        self.next_query_id = self.next_query_id.wrapping_add(1);

        self.pending_sessions.insert(
            id,
            PendingSessionQuery {
                reply,
                collected,
                awaiting,
                deadline: Instant::now() + SESSION_QUERY_TIMEOUT,
            },
        );

        /*
          Sessions are read straight from each node's SessionService, so this is
          a plain broadcast with the answers merged as they arrive.
        */
        let entries = self.membership.to_entries(&self.self_desc);
        self.broadcast(ClusterMessage::Membership { entries });
        self.resolve_session_query(id);
    }

    fn resolve_session_query(&mut self, id: u64) {
        let Some(query) = self.pending_sessions.get(&id) else {
            return;
        };

        if !query.awaiting.is_empty() && Instant::now() < query.deadline {
            return;
        }

        if let Some(mut query) = self.pending_sessions.remove(&id) {
            query.collected.sort_by(|a, b| a.client_id.cmp(&b.client_id));
            let _ = query.reply.send(query.collected);
        }
    }

    /*
      Answer any session query that has run out of time. A node that never
      replies must not leave the HTTP request hanging.
    */
    fn expire_session_queries(&mut self) {
        let now = Instant::now();
        let expired: Vec<u64> = self
            .pending_sessions
            .iter()
            .filter(|(_, q)| now >= q.deadline || q.awaiting.is_empty())
            .map(|(id, _)| *id)
            .collect();

        for id in expired {
            if let Some(mut query) = self.pending_sessions.remove(&id) {
                query.collected.sort_by(|a, b| a.client_id.cmp(&b.client_id));
                let _ = query.reply.send(query.collected);
            }
        }
    }

    fn status(&self) -> ClusterStatus {
        let alive = self
            .membership
            .all()
            .filter(|m| m.state == MemberState::Alive)
            .count();

        ClusterStatus {
            enabled: true,
            cluster: self.self_desc.cluster.clone(),
            node_id: self.self_desc.id.to_string(),
            advertise_addr: self.self_desc.advertise_addr.to_string(),
            incarnation: self.self_desc.incarnation,
            discovery: self.discovery.name().to_string(),
            /* Plus one for ourselves, who are never in the member table. */
            members_total: self.membership.all().count() + 1,
            members_alive: alive + 1,
            routes_total: self.routes.total_entries() + self.local_filters.len(),
            federation_links: self.federation.all().count(),
            is_federation_owner: Federation::is_owner(
                &self.self_desc.id,
                &self.membership.alive_ids(),
            ),
        }
    }

    fn nodes(&self) -> Vec<NodeView> {
        let mut out = vec![NodeView {
            id: self.self_desc.id.to_string(),
            cluster: self.self_desc.cluster.clone(),
            advertise_addr: self.self_desc.advertise_addr.to_string(),
            api_addr: self.self_desc.api_addr.map(|a| a.to_string()),
            state: "alive".to_string(),
            incarnation: self.self_desc.incarnation,
            version: self.self_desc.version.clone(),
            last_seen_secs: 0,
            is_self: true,
            routes: self.local_filters.len(),
            dropped_messages: 0,
        }];

        for member in self.membership.all() {
            let dropped = self
                .peers
                .get(&member.desc.id)
                .map(|p| p.dropped)
                .unwrap_or(0);

            out.push(NodeView {
                id: member.desc.id.to_string(),
                cluster: member.desc.cluster.clone(),
                advertise_addr: member.desc.advertise_addr.to_string(),
                api_addr: member.desc.api_addr.map(|a| a.to_string()),
                state: member.state.as_str().to_string(),
                incarnation: member.desc.incarnation,
                version: member.desc.version.clone(),
                last_seen_secs: member.last_seen.elapsed().as_secs(),
                is_self: false,
                routes: self.routes.filters_for(&member.desc.id).len(),
                dropped_messages: dropped,
            });
        }

        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    fn send_to(&mut self, node: &NodeId, msg: ClusterMessage) {
        if let Some(peer) = self.peers.get_mut(node) {
            peer.try_send(msg);
        }
    }

    fn broadcast(&mut self, msg: ClusterMessage) {
        for peer in self.peers.values_mut() {
            peer.try_send(msg.clone());
        }
    }

    fn broadcast_leave(&mut self) {
        let msg = ClusterMessage::Leave {
            node: self.self_desc.id.clone(),
            reason: LeaveReason::Shutdown,
        };
        self.broadcast(msg);
    }
}
