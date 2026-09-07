use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::cluster::node::NodeId;
use crate::cluster::protocol::MetaTable;
use crate::storage::redb::auth::{AUTH_CONFIG, MQTT_CREDS};
use crate::storage::redb::listener::LISTENERS;
use crate::storage::redb::user::USERS;
use crate::storage::redb::webhook::WEBHOOKS;

/* Node identity and incarnation, so a restart keeps the same id. */
pub const CLUSTER_STATE: TableDefinition<&str, &[u8]> = TableDefinition::new("cluster_state");

/*
  Per-record replication metadata, keyed "<table>:<key>".

  Kept in its own table rather than embedded in each model so the replicated
  payload stays byte-identical to what the repos already store, and so adding
  replication does not migrate any existing data.
*/
pub const META_VERSIONS: TableDefinition<&str, &[u8]> = TableDefinition::new("meta_versions");

const NODE_ID_KEY: &str = "node_id";
const INCARNATION_KEY: &str = "incarnation";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaVersion {
    pub updated_at: i64,
    pub updated_by: NodeId,
    /* A tombstone: the record is gone but its version must outlive it. */
    pub deleted: bool,
}

#[derive(Clone)]
pub struct ClusterRepo {
    db: Arc<Database>,
}

impl ClusterRepo {
    pub fn new(db: Arc<Database>) -> Self {
        let write_txn = db.begin_write().expect("begin write txn for cluster tables");
        {
            let _ = write_txn.open_table(CLUSTER_STATE).expect("open cluster_state table");
            let _ = write_txn.open_table(META_VERSIONS).expect("open meta_versions table");
        }
        write_txn.commit().expect("commit cluster table init");
        Self { db }
    }

    pub fn get_node_id(&self) -> Result<Option<String>> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CLUSTER_STATE)?;
        match table.get(NODE_ID_KEY)? {
            Some(v) => Ok(Some(String::from_utf8_lossy(v.value()).to_string())),
            None => Ok(None),
        }
    }

    pub fn set_node_id(&self, id: &str) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CLUSTER_STATE)?;
            table.insert(NODE_ID_KEY, id.as_bytes())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /*
      Increment and return the incarnation. Called once per process start; peers
      use the increase to recognise a node that restarted between their probes.
    */
    pub fn bump_incarnation(&self) -> Result<u64> {
        let write_txn = self.db.begin_write()?;
        let next;
        {
            let mut table = write_txn.open_table(CLUSTER_STATE)?;
            let current = match table.get(INCARNATION_KEY)? {
                Some(v) => {
                    let bytes = v.value();
                    if bytes.len() == 8 {
                        u64::from_be_bytes(bytes.try_into().unwrap_or([0; 8]))
                    } else {
                        0
                    }
                }
                None => 0,
            };
            next = current.saturating_add(1);
            table.insert(INCARNATION_KEY, next.to_be_bytes().as_slice())?;
        }
        write_txn.commit()?;
        Ok(next)
    }

    fn version_key(table: MetaTable, key: &str) -> String {
        format!("{}:{}", table.as_str(), key)
    }

    pub fn get_version(&self, table: MetaTable, key: &str) -> Result<Option<MetaVersion>> {
        let read_txn = self.db.begin_read()?;
        let versions = read_txn.open_table(META_VERSIONS)?;
        match versions.get(Self::version_key(table, key).as_str())? {
            Some(v) => Ok(Some(bincode::deserialize(v.value())?)),
            None => Ok(None),
        }
    }

    pub fn put_version(&self, table: MetaTable, key: &str, version: &MetaVersion) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut versions = write_txn.open_table(META_VERSIONS)?;
            let bytes = bincode::serialize(version)?;
            versions.insert(Self::version_key(table, key).as_str(), bytes.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn all_versions(&self) -> Result<Vec<(MetaTable, String, MetaVersion)>> {
        let read_txn = self.db.begin_read()?;
        let versions = read_txn.open_table(META_VERSIONS)?;

        let mut out = Vec::new();
        for entry in versions.iter()? {
            let (k, v) = entry?;
            let composite = k.value().to_string();
            let Some((table_name, key)) = composite.split_once(':') else {
                continue;
            };
            let Some(table) = table_from_str(table_name) else {
                continue;
            };
            let version: MetaVersion = bincode::deserialize(v.value())?;
            out.push((table, key.to_string(), version));
        }
        Ok(out)
    }

    /*
      Hard-delete tombstones older than the TTL. Until then they must stay: a
      tombstone that disappears too early lets any peer still holding the old
      value resurrect the record.
    */
    pub fn sweep_tombstones(&self, ttl: Duration, now_ms: i64) -> Result<usize> {
        let cutoff = now_ms - ttl.as_millis() as i64;
        let mut expired = Vec::new();

        {
            let read_txn = self.db.begin_read()?;
            let versions = read_txn.open_table(META_VERSIONS)?;
            for entry in versions.iter()? {
                let (k, v) = entry?;
                let version: MetaVersion = bincode::deserialize(v.value())?;
                if version.deleted && version.updated_at < cutoff {
                    expired.push(k.value().to_string());
                }
            }
        }

        if expired.is_empty() {
            return Ok(0);
        }

        let write_txn = self.db.begin_write()?;
        {
            let mut versions = write_txn.open_table(META_VERSIONS)?;
            for key in &expired {
                versions.remove(key.as_str())?;
            }
        }
        write_txn.commit()?;

        Ok(expired.len())
    }

    /*
      Raw access to the replicated tables. The bytes are exactly what the owning
      repo serialized, so replication never needs to understand the models.
    */
    pub fn raw_put(&self, table: MetaTable, key: &str, value: &[u8]) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            /*
              Each arm binds the table to a local so it outlives the insert; a
              temporary would be dropped before the borrow ends.
            */
            match table {
                MetaTable::User => {
                    let mut t = write_txn.open_table(USERS)?;
                    t.insert(key, value)?;
                }
                MetaTable::Listener => {
                    let mut t = write_txn.open_table(LISTENERS)?;
                    t.insert(key, value)?;
                }
                MetaTable::Webhook => {
                    let mut t = write_txn.open_table(WEBHOOKS)?;
                    t.insert(key, value)?;
                }
                MetaTable::AuthConfig => {
                    let mut t = write_txn.open_table(AUTH_CONFIG)?;
                    t.insert(key, value)?;
                }
                MetaTable::Credential => {
                    let mut t = write_txn.open_table(MQTT_CREDS)?;
                    t.insert(key, value)?;
                }
            }
        }
        write_txn.commit()?;
        Ok(())
    }

    pub fn raw_delete(&self, table: MetaTable, key: &str) -> Result<bool> {
        let write_txn = self.db.begin_write()?;
        let existed;
        {
            existed = match table {
                MetaTable::User => {
                    let mut t = write_txn.open_table(USERS)?;
                    t.remove(key)?.is_some()
                }
                MetaTable::Listener => {
                    let mut t = write_txn.open_table(LISTENERS)?;
                    t.remove(key)?.is_some()
                }
                MetaTable::Webhook => {
                    let mut t = write_txn.open_table(WEBHOOKS)?;
                    t.remove(key)?.is_some()
                }
                MetaTable::AuthConfig => {
                    let mut t = write_txn.open_table(AUTH_CONFIG)?;
                    t.remove(key)?.is_some()
                }
                MetaTable::Credential => {
                    let mut t = write_txn.open_table(MQTT_CREDS)?;
                    t.remove(key)?.is_some()
                }
            };
        }
        write_txn.commit()?;
        Ok(existed)
    }

    pub fn raw_get(&self, table: MetaTable, key: &str) -> Result<Option<Vec<u8>>> {
        let read_txn = self.db.begin_read()?;
        let value = match table {
            MetaTable::User => read_txn.open_table(USERS)?.get(key)?.map(|v| v.value().to_vec()),
            MetaTable::Listener => read_txn.open_table(LISTENERS)?.get(key)?.map(|v| v.value().to_vec()),
            MetaTable::Webhook => read_txn.open_table(WEBHOOKS)?.get(key)?.map(|v| v.value().to_vec()),
            MetaTable::AuthConfig => read_txn.open_table(AUTH_CONFIG)?.get(key)?.map(|v| v.value().to_vec()),
            MetaTable::Credential => read_txn.open_table(MQTT_CREDS)?.get(key)?.map(|v| v.value().to_vec()),
        };
        Ok(value)
    }
}

fn table_from_str(name: &str) -> Option<MetaTable> {
    match name {
        "user" => Some(MetaTable::User),
        "listener" => Some(MetaTable::Listener),
        "webhook" => Some(MetaTable::Webhook),
        "auth_config" => Some(MetaTable::AuthConfig),
        "credential" => Some(MetaTable::Credential),
        _ => None,
    }
}
