use std::sync::Arc;

use anyhow::Result;

use crate::cluster::meta::Replication;
use crate::cluster::protocol::MetaTable;
use redb::{Database, ReadableTable, TableDefinition};

use crate::models::auth::{AuthConfig, MqttCredential};

pub const AUTH_CONFIG: TableDefinition<&str, &[u8]> = TableDefinition::new("auth_config");
pub const MQTT_CREDS: TableDefinition<&str, &[u8]> = TableDefinition::new("mqtt_credentials");

const CONFIG_KEY: &str = "config";

#[derive(Clone)]
pub struct AuthRepo {
    db: Arc<Database>,
    replication: Option<Arc<Replication>>,
}

impl AuthRepo {
    /*
      Attach the cluster replication hook. Absent on a single-node broker, in
      which case writes are neither stamped nor announced.
    */
    pub fn set_replication(&mut self, replication: Option<Arc<Replication>>) {
        self.replication = replication;
    }

    pub fn new(db: Arc<Database>) -> Self {
        let write_txn = db.begin_write().expect("begin write txn for auth tables");
        {
            let _ = write_txn.open_table(AUTH_CONFIG).expect("open auth_config table");
            let _ = write_txn.open_table(MQTT_CREDS).expect("open mqtt_credentials table");
        }
        write_txn.commit().expect("commit auth table init");
        Self { db, replication: None }
    }

    pub fn get_config(&self) -> Result<AuthConfig> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(AUTH_CONFIG)?;
        match table.get(CONFIG_KEY)? {
            Some(value) => Ok(bincode::deserialize(value.value())?),
            None => Ok(AuthConfig::default()),
        }
    }

    pub fn set_config(&self, cfg: &AuthConfig) -> Result<()> {
        let bytes = bincode::serialize(cfg)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(AUTH_CONFIG)?;
            table.insert(CONFIG_KEY, bytes.as_slice())?;
        }
        write_txn.commit()?;

        if let Some(r) = &self.replication {
            r.record(MetaTable::AuthConfig, CONFIG_KEY, Some(bytes));
        }
        Ok(())
    }

    pub fn cred_upsert(&self, cred: &MqttCredential) -> Result<()> {
        let bytes = bincode::serialize(cred)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(MQTT_CREDS)?;
            table.insert(cred.username.as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;

        /* Password hashes replicate as opaque bytes; nothing decodes them here. */
        if let Some(r) = &self.replication {
            r.record(MetaTable::Credential, &cred.username, Some(bytes));
        }
        Ok(())
    }

    pub fn cred_get(&self, username: &str) -> Result<Option<MqttCredential>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(MQTT_CREDS)?;
        match table.get(username)? {
            Some(value) => Ok(Some(bincode::deserialize(value.value())?)),
            None => Ok(None),
        }
    }

    pub fn cred_delete(&self, username: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let existed;
        {
            let mut table = write_txn.open_table(MQTT_CREDS)?;
            existed = table.remove(username)?.is_some();
        }
        write_txn.commit()?;

        if existed {
            if let Some(r) = &self.replication {
                r.record(MetaTable::Credential, username, None);
            }
        }
        Ok(existed)
    }

    pub fn cred_all(&self) -> Result<Vec<MqttCredential>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(MQTT_CREDS)?;
        let mut creds = Vec::new();
        for entry in table.iter()? {
            let (_k, v) = entry?;
            creds.push(bincode::deserialize(v.value())?);
        }
        Ok(creds)
    }
}
