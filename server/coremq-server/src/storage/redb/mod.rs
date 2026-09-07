use std::sync::Arc;

use redb::Database;

use crate::cluster::meta::Replication;
use crate::storage::redb::auth::AuthRepo;
use crate::storage::redb::listener::ListenerRepo;
use crate::storage::redb::user::UserRepo;
use crate::storage::redb::webhook::WebhookRepo;

pub mod user;
pub mod webhook;
pub mod listener;
pub mod auth;
pub mod cluster;

#[derive(Clone)]
pub struct Storage {
    pub user: UserRepo,
    pub webhook: WebhookRepo,
    pub listener: ListenerRepo,
    pub auth: AuthRepo,
}


impl Storage {
    pub fn new(db: Arc<Database>) -> Self {
        Self::with_replication(db, None)
    }

    /*
      Build the repos with an optional cluster replication hook. Passing None
      gives exactly the previous single-node behaviour: no version rows, no
      broadcast, no overhead on any write.
    */
    pub fn with_replication(db: Arc<Database>, replication: Option<Arc<Replication>>) -> Self {
        let mut user = UserRepo::new(db.clone());
        let mut webhook = WebhookRepo::new(db.clone());
        let mut listener = ListenerRepo::new(db.clone());
        let mut auth = AuthRepo::new(db);

        user.set_replication(replication.clone());
        webhook.set_replication(replication.clone());
        listener.set_replication(replication.clone());
        auth.set_replication(replication);

        Self {
            user,
            webhook,
            listener,
            auth,
        }
    }
}