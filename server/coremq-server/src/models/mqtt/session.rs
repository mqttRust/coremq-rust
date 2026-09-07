use std::{collections::HashMap, net::SocketAddr};
use chrono::{DateTime, Local};
use serde::Serialize;
use tokio::sync::{ mpsc};

use crate::{
    enums::MqttChannel, 
    utils::format_time::format_datetime,
    protocol::packets::{ SubscribePacket}};


#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub client_id: String,
    pub username: String,
    pub clean_session: bool,
    pub remote_addr: SocketAddr,
    pub connected_port: u16,

    /*
      Which node owns this session. Blank on a single-node broker; the dashboard
      shows it as a column once clustering is on.
    */
    #[serde(default)]
    pub node_id: String,

    #[serde(serialize_with = "format_datetime")]
    pub connected_at: DateTime<Local>,
    pub subscriptions: HashMap<String, SubscribePacket>,

     #[serde(skip)]
    pub tx: mpsc::Sender<MqttChannel>,
}

impl Session {
    pub fn new(
        client_id: String,
        username: String,
        clean_session: bool,
        connected_port: u16,
        remote_addr: SocketAddr,
        node_id: String,
        tx: mpsc::Sender<MqttChannel>,
    ) -> Self {
        Self {
            client_id,
            username,
            clean_session,
            connected_port,
            node_id,
            connected_at: Local::now(),
            subscriptions: HashMap::new(),
            remote_addr,
            tx,
        }
    }

    pub fn add_subscription(&mut self, sub: SubscribePacket) {
        self.subscriptions.insert(sub.topic.clone(), sub);
    }

    pub fn remove_subscription(&mut self, topic: &str) {
        self.subscriptions.remove(topic);
    }
}


