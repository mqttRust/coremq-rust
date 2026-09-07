use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use tokio::sync::mpsc;

use crate::cluster::node::NodeId;
use crate::cluster::protocol::{MetaEntry, MetaTable};
use crate::storage::redb::cluster::{ClusterRepo, MetaVersion};

/*
  Local-change queue depth. Metadata writes are admin actions, not a hot path,
  so a shallow bounded queue is plenty and keeps a stalled cluster runtime from
  growing this without limit.
*/
pub const META_QUEUE_DEPTH: usize = 256;

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/*
  Attached to the storage repos so every local write is stamped and announced.

  When clustering is off this is simply absent, and the repos behave exactly as
  they did before — no version rows, no channel, no overhead.
*/
pub struct Replication {
    node_id: NodeId,
    repo: ClusterRepo,
    tx: mpsc::Sender<MetaEntry>,
}

impl Replication {
    pub fn new(node_id: NodeId, repo: ClusterRepo) -> (Arc<Self>, mpsc::Receiver<MetaEntry>) {
        let (tx, rx) = mpsc::channel(META_QUEUE_DEPTH);
        (Arc::new(Self { node_id, repo, tx }), rx)
    }

    pub fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /*
      Record a local write and queue it for broadcast. Called by the repos after
      they have committed, so a failed write never replicates.
    */
    pub fn record(&self, table: MetaTable, key: &str, value: Option<Vec<u8>>) {
        let entry = MetaEntry {
            table,
            key: key.to_string(),
            value,
            updated_at: now_ms(),
            updated_by: self.node_id.clone(),
        };

        let version = MetaVersion {
            updated_at: entry.updated_at,
            updated_by: entry.updated_by.clone(),
            deleted: entry.value.is_none(),
        };

        if let Err(e) = self.repo.put_version(table, key, &version) {
            println!("cluster: failed to record meta version for {}:{}: {}", table.as_str(), key, e);
            return;
        }

        /*
          A full queue means the cluster runtime is wedged. Dropping is correct:
          the periodic MetaSyncRequest reconciliation will carry the change.
        */
        if self.tx.try_send(entry).is_err() {
            println!("cluster: meta queue full, {}:{} will replicate on next sync", table.as_str(), key);
        }
    }
}

/*
  Applies inbound metadata deltas under last-write-wins.

  Returns whether anything changed, so the runtime can reload the services that
  cache config (auth in particular) only when it actually needs to.
*/
pub struct MetaApplier {
    repo: ClusterRepo,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub applied: usize,
    pub rejected: usize,
    pub auth_changed: bool,
}

impl MetaApplier {
    pub fn new(repo: ClusterRepo) -> Self {
        Self { repo }
    }

    pub fn apply(&self, entries: Vec<MetaEntry>) -> ApplyOutcome {
        let mut outcome = ApplyOutcome::default();

        for entry in entries {
            match self.apply_one(&entry) {
                Ok(true) => {
                    outcome.applied += 1;
                    if matches!(entry.table, MetaTable::AuthConfig | MetaTable::Credential) {
                        outcome.auth_changed = true;
                    }
                }
                Ok(false) => outcome.rejected += 1,
                Err(e) => {
                    println!(
                        "cluster: failed to apply {}:{}: {}",
                        entry.table.as_str(),
                        entry.key,
                        e
                    );
                    outcome.rejected += 1;
                }
            }
        }

        outcome
    }

    fn apply_one(&self, entry: &MetaEntry) -> Result<bool> {
        let current = self.repo.get_version(entry.table, &entry.key)?;

        if let Some(current) = current {
            let local = MetaEntry {
                table: entry.table,
                key: entry.key.clone(),
                value: None,
                updated_at: current.updated_at,
                updated_by: current.updated_by.clone(),
            };

            /* Our copy is newer, or the same write echoed back. Ignore it. */
            if !entry.supersedes(&local) {
                return Ok(false);
            }
        }

        match &entry.value {
            Some(bytes) => self.repo.raw_put(entry.table, &entry.key, bytes)?,
            None => {
                self.repo.raw_delete(entry.table, &entry.key)?;
            }
        }

        self.repo.put_version(
            entry.table,
            &entry.key,
            &MetaVersion {
                updated_at: entry.updated_at,
                updated_by: entry.updated_by.clone(),
                deleted: entry.value.is_none(),
            },
        )?;

        Ok(true)
    }

    /*
      Hard-delete tombstones past their TTL. Until then they must survive, or a
      peer still holding the old value resurrects the record.
    */
    pub fn sweep_tombstones(&self, ttl: std::time::Duration) -> Result<usize> {
        self.repo.sweep_tombstones(ttl, now_ms())
    }

    /*
      Everything this node holds, for a full sync with a peer that just joined
      or reconnected. Tombstones are included — a peer that never heard the
      delete would otherwise resurrect the record.
    */
    pub fn snapshot(&self) -> Result<Vec<MetaEntry>> {
        let mut out = Vec::new();

        for (table, key, version) in self.repo.all_versions()? {
            let value = if version.deleted {
                None
            } else {
                match self.repo.raw_get(table, &key)? {
                    Some(bytes) => Some(bytes),
                    /*
                      Version says present but the row is gone. Treat it as a
                      tombstone rather than announcing a value we cannot produce.
                    */
                    None => None,
                }
            };

            out.push(MetaEntry {
                table,
                key,
                value,
                updated_at: version.updated_at,
                updated_by: version.updated_by,
            });
        }

        Ok(out)
    }
}

/*
  Wall-clock skew between two nodes. LWW compares timestamps across machines, so
  a node whose clock runs fast silently wins every conflict.
*/
pub fn skew_warning(peer_started_at: i64, threshold_ms: i64) -> Option<String> {
    if peer_started_at == 0 {
        return None;
    }

    let skew = (now_ms() - peer_started_at).abs();
    if skew > threshold_ms {
        return Some(format!(
            "peer clock differs by {}ms; last-write-wins needs NTP to be correct",
            skew
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pkg;

    fn temp_repo() -> (ClusterRepo, tempdir::TempDirGuard) {
        let guard = tempdir::TempDirGuard::new();
        let db = pkg::db::new(&guard.db_path()).expect("open temp db");
        (ClusterRepo::new(Arc::new(db)), guard)
    }

    /*
      Minimal scratch-directory helper so these tests do not need a dev-dependency.
    */
    mod tempdir {
        use std::path::PathBuf;

        pub struct TempDirGuard {
            dir: PathBuf,
        }

        impl TempDirGuard {
            pub fn new() -> Self {
                let unique = format!(
                    "coremq-meta-test-{}-{:?}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0)
                );
                let dir = std::env::temp_dir().join(unique);
                std::fs::create_dir_all(&dir).expect("create temp dir");
                Self { dir }
            }

            pub fn db_path(&self) -> String {
                self.dir.join("test.redb").to_string_lossy().to_string()
            }
        }

        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.dir);
            }
        }
    }

    fn entry(key: &str, value: Option<Vec<u8>>, at: i64, by: &str) -> MetaEntry {
        MetaEntry {
            table: MetaTable::User,
            key: key.to_string(),
            value,
            updated_at: at,
            updated_by: NodeId::new(by),
        }
    }

    #[test]
    fn a_newer_write_is_applied() {
        let (repo, _guard) = temp_repo();
        let applier = MetaApplier::new(repo);

        let outcome = applier.apply(vec![entry("alice", Some(vec![1]), 100, "a")]);
        assert_eq!(outcome.applied, 1);

        let outcome = applier.apply(vec![entry("alice", Some(vec![2]), 200, "a")]);
        assert_eq!(outcome.applied, 1);
    }

    #[test]
    fn an_older_write_is_rejected() {
        let (repo, _guard) = temp_repo();
        let applier = MetaApplier::new(repo.clone());

        applier.apply(vec![entry("alice", Some(vec![2]), 200, "a")]);
        let outcome = applier.apply(vec![entry("alice", Some(vec![1]), 100, "a")]);

        assert_eq!(outcome.applied, 0);
        assert_eq!(outcome.rejected, 1);
        assert_eq!(repo.raw_get(MetaTable::User, "alice").unwrap(), Some(vec![2]));
    }

    #[test]
    fn concurrent_writes_converge_on_the_same_winner() {
        let (repo_a, _ga) = temp_repo();
        let (repo_b, _gb) = temp_repo();

        let from_a = entry("alice", Some(vec![b'a']), 500, "node-a");
        let from_b = entry("alice", Some(vec![b'b']), 500, "node-b");

        /* Same timestamp, opposite arrival order on the two nodes. */
        MetaApplier::new(repo_a.clone()).apply(vec![from_a.clone(), from_b.clone()]);
        MetaApplier::new(repo_b.clone()).apply(vec![from_b, from_a]);

        let a = repo_a.raw_get(MetaTable::User, "alice").unwrap();
        let b = repo_b.raw_get(MetaTable::User, "alice").unwrap();
        assert_eq!(a, b, "nodes diverged on a tie");
        assert_eq!(a, Some(vec![b'b']), "the larger node id should win the tie");
    }

    #[test]
    fn a_tombstone_deletes_and_survives_a_stale_resurrect() {
        let (repo, _guard) = temp_repo();
        let applier = MetaApplier::new(repo.clone());

        applier.apply(vec![entry("alice", Some(vec![1]), 100, "a")]);
        applier.apply(vec![entry("alice", None, 200, "a")]);
        assert_eq!(repo.raw_get(MetaTable::User, "alice").unwrap(), None);

        /* A peer that never heard the delete replays the old value. */
        let outcome = applier.apply(vec![entry("alice", Some(vec![1]), 150, "b")]);
        assert_eq!(outcome.applied, 0);
        assert_eq!(repo.raw_get(MetaTable::User, "alice").unwrap(), None);
    }

    #[test]
    fn snapshot_includes_tombstones() {
        let (repo, _guard) = temp_repo();
        let applier = MetaApplier::new(repo);

        applier.apply(vec![entry("alice", Some(vec![1]), 100, "a")]);
        applier.apply(vec![entry("bob", None, 100, "a")]);

        let snapshot = applier.snapshot().unwrap();
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.iter().any(|e| e.key == "bob" && e.value.is_none()));
    }

    #[test]
    fn tombstones_expire_only_after_the_ttl() {
        let (repo, _guard) = temp_repo();
        let applier = MetaApplier::new(repo.clone());

        applier.apply(vec![entry("gone", None, 1_000, "a")]);

        let ttl = std::time::Duration::from_millis(500);
        assert_eq!(repo.sweep_tombstones(ttl, 1_200).unwrap(), 0);
        assert_eq!(repo.sweep_tombstones(ttl, 2_000).unwrap(), 1);
    }

    #[test]
    fn auth_changes_are_flagged_so_the_cache_can_reload() {
        let (repo, _guard) = temp_repo();
        let applier = MetaApplier::new(repo);

        let outcome = applier.apply(vec![MetaEntry {
            table: MetaTable::AuthConfig,
            key: "config".into(),
            value: Some(vec![1]),
            updated_at: 100,
            updated_by: NodeId::new("a"),
        }]);

        assert!(outcome.auth_changed);
    }
}
