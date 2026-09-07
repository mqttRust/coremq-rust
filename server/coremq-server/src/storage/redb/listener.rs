use std::sync::Arc;

use anyhow::Result;

use crate::cluster::meta::Replication;
use crate::cluster::protocol::MetaTable;
use redb::{Database, ReadableTable, TableDefinition};

use crate::models::listener::ListenerConfig;

pub const LISTENERS: TableDefinition<&str, &[u8]> = TableDefinition::new("listeners");

#[derive(Clone)]
pub struct ListenerRepo {
    db: Arc<Database>,
    replication: Option<Arc<Replication>>,
}

impl ListenerRepo {
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
            .open_table(LISTENERS)
            .expect("Failed to create/open LISTENERS table");
        write_txn.commit().expect("Failed to commit table init");
        Self { db, replication: None }
    }

    pub fn upsert(&self, cfg: &ListenerConfig) -> Result<()> {
        let bytes = bincode::serialize(cfg)?;
        let key = cfg.port.to_string();
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LISTENERS)?;
            table.insert(key.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;

        if let Some(r) = &self.replication {
            r.record(MetaTable::Listener, &key, Some(bytes));
        }
        Ok(())
    }

    pub fn delete(&self, port: u16) -> Result<()> {
        let key = port.to_string();
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(LISTENERS)?;
            table.remove(key.as_str())?;
        }
        write_txn.commit()?;

        if let Some(r) = &self.replication {
            r.record(MetaTable::Listener, &key, None);
        }
        Ok(())
    }

    pub fn get_all(&self) -> Result<Vec<ListenerConfig>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(LISTENERS)?;

        let mut listeners = Vec::new();
        for entry in table.iter()? {
            let (_key, value) = entry?;
            let cfg: ListenerConfig = bincode::deserialize(value.value())?;
            listeners.push(cfg);
        }
        Ok(listeners)
    }
}
