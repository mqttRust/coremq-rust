use std::{collections::HashMap, sync::Arc};
use tokio::{sync::watch, task::JoinHandle};


use crate::{
    cluster::{ClusterHandle, Origin},
    engine::{AdminCommand, ConnectCommand, EngineChannels, PubSubCommand},
    enums::MqttChannel, models::{config::Config, listener::ListenerConfig, listener_status::ListenerStatus, pagination::Page, session::Session, webhook::{events, WebhookEvent}},
    protocol::packets::PublishPacket, services::{webhook::WebhookDispatcher, SessionService, TopicService}, transport::ProtocolState
};

pub struct Engine {
    client_service: Arc<SessionService>,
    topic_service: TopicService,
    channels: EngineChannels,
    webhook: Arc<WebhookDispatcher>,
   pub listeners: HashMap<u16, (JoinHandle<()>, watch::Sender<bool>, ListenerConfig)>,
   pub protocol_state: Option<Arc<ProtocolState>>,
   pub config: Config,
   pub cluster: Option<ClusterHandle>,
}

impl Engine {
    pub fn new(
        client_service: Arc<SessionService>,
        config: Config,
        channels: EngineChannels,
        webhook: Arc<WebhookDispatcher>,
    ) -> Self {
        Self {
            topic_service: TopicService::new(),
            listeners: HashMap::new(),
            protocol_state: None,
            client_service,
            channels,
            webhook,
            config: config,
            cluster: None,
        }
    }

    pub fn with_cluster(mut self, cluster: Option<ClusterHandle>) -> Self {
        self.cluster = cluster;
        self
    }

    fn node_id(&self) -> String {
        self.cluster
            .as_ref()
            .map(|c| c.node_id().to_string())
            .unwrap_or_default()
    }

    pub fn drop_client(&mut self, client_id: &str) {
        if let Some(_) = self.client_service.remove_client(client_id) {
            /*
              Collect the filters this client was the last holder of *before*
              removing it, then announce the RouteDels once it is gone.
            */
            let orphaned = match &self.cluster {
                Some(_) => self.topic_service.filters_orphaned_by(client_id),
                None => Vec::new(),
            };

            self.topic_service.remove_client(client_id);

            if let Some(cluster) = &self.cluster {
                for filter in orphaned {
                    cluster.route_del(&filter);
                }
            }
        }
    }

    fn publish(&self, p: PublishPacket, origin: Origin) {
        if self.webhook.any_enabled_for(events::MESSAGE_PUBLISHED) {
            let mut ev = WebhookEvent::new(events::MESSAGE_PUBLISHED);
            ev.topic = Some(p.topic.clone());
            ev.payload = Some(String::from_utf8_lossy(&p.payload).to_string());
            ev.qos = Some(p.qos);
            ev.retain = Some(p.retain);
            self.webhook.dispatch(ev);
        }

        let subscribers = self.topic_service.match_subscribers(&p.topic);
        for client_id in subscribers {
            if let Some(session) = self.client_service.get_session(&client_id) {
                let _ = session.tx.try_send(MqttChannel::Publish(p.clone()));
            }
        }

        /*
          Only a message from a local client is fanned out to the cluster. One
          that arrived from a peer has already been delivered everywhere by its
          origin node, so forwarding it again would loop.
        */
        if origin.is_local() {
            if let Some(cluster) = &self.cluster {
                cluster.forward(p);
            }
        }
    }

    pub fn get_paginated(&self, page: usize, size: usize) -> Page<Session> {
        self.client_service.get_paginated(page, size)
    }

    pub fn get_listeners(&self) -> Vec<ListenerStatus> {
        self.listeners
            .values()
            .map(|(_, _, config)| ListenerStatus {
                connections: self.client_service.get_by_listener(config.port).len(),
                config: config.clone(),
            })
            .collect()
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                Some(cmd) = self.channels.connect_rx.recv() => {
                    match cmd {
                        ConnectCommand::Connect(packet, port, remote_addr,   tx) => {
                            let old_session = self.client_service.remove_client(&packet.client_id);
                            if let Some(session) = old_session {
                                self.topic_service.remove_client(&session.client_id);
                                let _ = session.tx.send(MqttChannel::Disconnect).await;
                            }

                            // Enforce the listener's connection limit, if one is set.
                            let at_limit = self
                                .listeners
                                .get(&port)
                                .and_then(|(_, _, cfg)| cfg.max_connections)
                                .map(|max| self.client_service.get_by_listener(port).len() as u32 >= max)
                                .unwrap_or(false);

                            if at_limit {
                                println!("Listener on port {} at connection limit; rejecting client {}", port, packet.client_id);
                                let _ = tx.send(MqttChannel::Disconnect).await;
                            } else {
                                println!("Clinet connected: {:?}", packet);

                                let node_id = self.node_id();
                                self.client_service.add_client(&packet, port, remote_addr, node_id, tx);

                                /*
                                  MQTT requires a client id to be unique cluster-wide,
                                  so tell the other nodes to drop any copy they hold.
                                */
                                if let Some(cluster) = &self.cluster {
                                    cluster.claim_session(&packet.client_id);
                                }

                                let mut ev = WebhookEvent::new(events::CLIENT_CONNECTED);
                                ev.client_id = Some(packet.client_id.clone());
                                ev.username = packet.username.clone();
                                ev.remote_addr = Some(remote_addr.to_string());
                                self.webhook.dispatch(ev);
                            }
                        }
                        ConnectCommand::Disconnect(client_id) => {
                            self.drop_client(&client_id);

                            let mut ev = WebhookEvent::new(events::CLIENT_DISCONNECTED);
                            ev.client_id = Some(client_id.clone());
                            self.webhook.dispatch(ev);
                        }
                        ConnectCommand::Takeover(client_id) => {
                            if let Some(session) = self.client_service.remove_client(&client_id) {
                                let orphaned = self.topic_service.filters_orphaned_by(&client_id);
                                self.topic_service.remove_client(&client_id);

                                if let Some(cluster) = &self.cluster {
                                    for filter in orphaned {
                                        cluster.route_del(&filter);
                                    }
                                }

                                let _ = session.tx.send(MqttChannel::Disconnect).await;

                                let mut ev = WebhookEvent::new(events::CLIENT_DISCONNECTED);
                                ev.client_id = Some(client_id.clone());
                                self.webhook.dispatch(ev);
                            }
                        }
                    }
                }

                Some(cmd) = self.channels.pubsub_rx.recv() => {
                    match cmd {
                        PubSubCommand::Subscribe(packet, client_id) => {
                            self.client_service.add_subscribtion(&client_id, &packet);

                            /* Announce the route only on the 0 -> 1 edge. */
                            if self.topic_service.subscribe(&packet.topic, &client_id) {
                                if let Some(cluster) = &self.cluster {
                                    cluster.route_add(&packet.topic);
                                }
                            }

                            let mut ev = WebhookEvent::new(events::SUBSCRIPTION_CREATED);
                            ev.client_id = Some(client_id.clone());
                            ev.topic = Some(packet.topic.clone());
                            ev.qos = Some(packet.qos);
                            self.webhook.dispatch(ev);
                        }
                        PubSubCommand::Unsubscribe(packet, client_id) => {
                            self.client_service.remove_subscribtion(&client_id, &packet.topic);

                            /* Withdraw the route only on the 1 -> 0 edge. */
                            if self.topic_service.unsubscribe(&packet.topic, &client_id) {
                                if let Some(cluster) = &self.cluster {
                                    cluster.route_del(&packet.topic);
                                }
                            }

                            let mut ev = WebhookEvent::new(events::SUBSCRIPTION_REMOVED);
                            ev.client_id = Some(client_id.clone());
                            ev.topic = Some(packet.topic.clone());
                            self.webhook.dispatch(ev);
                        }
                        PubSubCommand::Publish(packet, origin) => {
                            self.publish(packet, origin);
                        }
                    }
                }

                Some(cmd) = self.channels.admin_rx.recv() => {
                    match cmd {
                        AdminCommand::GetClients(reply_tx, page, size) => {
                            let clients = self.get_paginated(page, size);
                            let _ = reply_tx.send(clients);
                        }

                        AdminCommand::StopListener(port) => {
                           let sessions = self.client_service.get_by_listener(port);
                           for s in sessions {
                             self.drop_client(&s.client_id);
                           }
                           
                           self.stop_listener(port).await;
                        }

                        AdminCommand::GetListeners(reply_tx) => {
                            let listeners = self.get_listeners();
                            let _ = reply_tx.send(listeners);
                        }

                        AdminCommand::StartListener(cfg, reply_tx) => {
                            let result = match self.protocol_state.clone() {
                                Some(state) => self.start_one(&cfg, state).await,
                                None => Err("listener subsystem not ready".to_string()),
                            };
                            let _ = reply_tx.send(result);
                        }

                        AdminCommand::DisconnectClient(client_id, reply_tx) => {
                            let existed = self.client_service.get_session(&client_id).is_some();
                            if existed {
                                self.drop_client(&client_id);
                            }
                            let _ = reply_tx.send(existed);
                        }

                        /*
                          Collect active topics and reply with results.
                        */
                        AdminCommand::GetTopics(reply_tx) => {
                            let topics = self.topic_service.collect_topics();
                            let _ = reply_tx.send(topics);
                        }

                        /*
                          Publish a message and acknowledge completion.
                        */
                        AdminCommand::PublishMessage(packet, reply_tx) => {
                            /* An API publish originates here, so it forwards like a client's. */
                            self.publish(packet, Origin::Local);
                            let _ = reply_tx.send(true);
                        }

                        AdminCommand::GetStats(reply_tx) => {
                            let count = self.client_service.client_count();
                            let topics = self.topic_service.collect_topics();
                            let _ = reply_tx.send((count, topics));
                        }
                    }
                }
            }
        }
    }
}
