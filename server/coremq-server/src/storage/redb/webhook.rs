use std::sync::Arc;

use anyhow::Result;

use crate::cluster::meta::Replication;
use crate::cluster::protocol::MetaTable;
use redb::{Database, ReadableTable, TableDefinition};

use crate::models::webhook::Webhook;

pub const WEBHOOKS: TableDefinition<&str, &[u8]> = TableDefinition::new("webhooks");

#[derive(Clone)]
pub struct WebhookRepo {
    db: Arc<Database>,
    replication: Option<Arc<Replication>>,
}

impl WebhookRepo {
    /*
      Attach the cluster replication hook. Absent on a single-node broker, in
      which case writes are neither stamped nor announced.
    */
    pub fn set_replication(&mut self, replication: Option<Arc<Replication>>) {
        self.replication = replication;
    }

    pub fn new(db: Arc<Database>) -> Self {
        let write_txn = db
            .begin_write()
            .expect("Failed to begin write txn for table init");
        let _ = write_txn
            .open_table(WEBHOOKS)
            .expect("Failed to create/open WEBHOOKS table");
        write_txn.commit().expect("Failed to commit table init");
        Self { db, replication: None }
    }

    pub fn upsert(&self, webhook: &Webhook) -> Result<()> {
        let bytes = bincode::serialize(webhook)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(WEBHOOKS)?;
            table.insert(webhook.id.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;

        /* Replicate only after the local commit succeeded. */
        if let Some(r) = &self.replication {
            r.record(MetaTable::Webhook, &webhook.id, Some(bytes));
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Webhook>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(WEBHOOKS)?;
        if let Some(value) = table.get(id)? {
            let webhook: Webhook = bincode::deserialize(value.value())?;
            Ok(Some(webhook))
        } else {
            Ok(None)
        }
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let existed;
        {
            let mut table = write_txn.open_table(WEBHOOKS)?;
            existed = table.remove(id)?.is_some();
        }
        write_txn.commit()?;

        if existed {
            if let Some(r) = &self.replication {
                r.record(MetaTable::Webhook, id, None);
            }
        }
        Ok(existed)
    }

    pub fn get_all(&self) -> Result<Vec<Webhook>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(WEBHOOKS)?;

        let mut webhooks = Vec::new();
        for entry in table.iter()? {
            let (_key, value) = entry?;
            let webhook: Webhook = bincode::deserialize(value.value())?;
            webhooks.push(webhook);
        }
        Ok(webhooks)
    }
}
