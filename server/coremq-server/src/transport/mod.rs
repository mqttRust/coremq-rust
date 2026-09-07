use std::sync::Arc;

use tokio::sync::mpsc;

use crate::engine::{ConnectCommand, PubSubCommand};
use crate::services::auth::AuthService;

pub mod ws;
pub mod tcp;
pub mod tls;

pub struct ProtocolState {
    pub connect_tx: mpsc::UnboundedSender<ConnectCommand>,
    pub pubsub_tx: mpsc::UnboundedSender<PubSubCommand>,
    pub auth: Arc<AuthService>,
}