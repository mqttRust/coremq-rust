use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::utils::duration::de_duration;

/*
  The default peer-mesh port. Deliberately outside the MQTT/admin range so a
  cluster port is never confused with a client-facing listener.
*/
pub const DEFAULT_CLUSTER_PORT: u16 = 4370;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_cluster_name")]
    pub name: String,

    /*
      Blank means "generate a UUID on first boot and persist it", so a node keeps
      its identity across restarts without the operator having to invent one.
    */
    #[serde(default)]
    pub node_id: String,

    #[serde(default = "default_bind")]
    pub bind: String,

    /*
      Address other nodes should dial. Blank means "derive from bind", which only
      works when bind names a routable interface.
    */
    #[serde(default)]
    pub advertise: String,

    /*
      Shared secret presented in Hello. Blank disables peer authentication, which
      is only safe on a trusted network.
    */
    #[serde(default)]
    pub secret: String,

    #[serde(default)]
    pub discovery: DiscoveryConfig,

    #[serde(default)]
    pub failure_detector: FailureDetectorConfig,

    #[serde(default)]
    pub cleanup: CleanupConfig,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            name: default_cluster_name(),
            node_id: String::new(),
            bind: default_bind(),
            advertise: String::new(),
            secret: String::new(),
            discovery: DiscoveryConfig::default(),
            failure_detector: FailureDetectorConfig::default(),
            cleanup: CleanupConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryType {
    Static,
    Dns,
    Multicast,
    K8s,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    #[serde(default = "default_discovery_type", rename = "type")]
    pub kind: DiscoveryType,

    /* static */
    #[serde(default)]
    pub seeds: Vec<String>,

    /* dns: a headless service name or any hostname with multiple A/AAAA records */
    #[serde(default)]
    pub query: String,

    /* dns / multicast / k8s: the peer port to assume for discovered hosts */
    #[serde(default = "default_peer_port")]
    pub port: u16,

    /* multicast */
    #[serde(default = "default_multicast_group")]
    pub group: String,

    #[serde(default = "default_multicast_port")]
    pub multicast_port: u16,

    /* k8s */
    #[serde(default)]
    pub namespace: String,

    #[serde(default)]
    pub label_selector: String,

    #[serde(default = "default_discovery_interval", deserialize_with = "de_duration")]
    pub interval: Duration,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            kind: default_discovery_type(),
            seeds: Vec::new(),
            query: String::new(),
            port: default_peer_port(),
            group: default_multicast_group(),
            multicast_port: default_multicast_port(),
            namespace: String::new(),
            label_selector: String::new(),
            interval: default_discovery_interval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureDetectorConfig {
    #[serde(default = "default_probe_interval", deserialize_with = "de_duration")]
    pub probe_interval: Duration,

    #[serde(default = "default_suspect_after", deserialize_with = "de_duration")]
    pub suspect_after: Duration,

    #[serde(default = "default_dead_after", deserialize_with = "de_duration")]
    pub dead_after: Duration,

    /*
      How many peers are asked to probe a target before it is suspected. Zero
      disables indirect probing, which makes a single bad link enough to suspect.
    */
    #[serde(default = "default_indirect_probes")]
    pub indirect_probes: usize,
}

impl Default for FailureDetectorConfig {
    fn default() -> Self {
        Self {
            probe_interval: default_probe_interval(),
            suspect_after: default_suspect_after(),
            dead_after: default_dead_after(),
            indirect_probes: default_indirect_probes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupConfig {
    #[serde(default = "default_reconcile_interval", deserialize_with = "de_duration")]
    pub reconcile_interval: Duration,

    #[serde(default = "default_orphan_sweep_interval", deserialize_with = "de_duration")]
    pub orphan_sweep_interval: Duration,

    #[serde(default = "default_tombstone_ttl", deserialize_with = "de_duration")]
    pub tombstone_ttl: Duration,
}

impl Default for CleanupConfig {
    fn default() -> Self {
        Self {
            reconcile_interval: default_reconcile_interval(),
            orphan_sweep_interval: default_orphan_sweep_interval(),
            tombstone_ttl: default_tombstone_ttl(),
        }
    }
}

/*
  A link to a different cluster. Unlike a peer, a federation endpoint never
  exchanges membership or metadata — only allowlisted traffic crosses.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationConfig {
    pub name: String,

    #[serde(default)]
    pub endpoints: Vec<String>,

    /* Filters whose matching messages we push to the remote cluster. */
    #[serde(default)]
    pub forward: Vec<String>,

    /* Filters we are willing to ingest from the remote cluster. */
    #[serde(default)]
    pub accept: Vec<String>,

    #[serde(default)]
    pub secret: String,

    #[serde(default = "default_federation_retry", deserialize_with = "de_duration")]
    pub retry_interval: Duration,
}

fn default_cluster_name() -> String {
    "default".to_string()
}

fn default_bind() -> String {
    format!("0.0.0.0:{}", DEFAULT_CLUSTER_PORT)
}

fn default_discovery_type() -> DiscoveryType {
    DiscoveryType::Static
}

fn default_peer_port() -> u16 {
    DEFAULT_CLUSTER_PORT
}

fn default_multicast_group() -> String {
    "239.255.70.70".to_string()
}

fn default_multicast_port() -> u16 {
    4371
}

fn default_discovery_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_probe_interval() -> Duration {
    Duration::from_secs(1)
}

fn default_suspect_after() -> Duration {
    Duration::from_secs(3)
}

fn default_dead_after() -> Duration {
    Duration::from_secs(10)
}

fn default_indirect_probes() -> usize {
    3
}

fn default_reconcile_interval() -> Duration {
    Duration::from_secs(30)
}

fn default_orphan_sweep_interval() -> Duration {
    Duration::from_secs(60)
}

fn default_tombstone_ttl() -> Duration {
    Duration::from_secs(24 * 3600)
}

fn default_federation_retry() -> Duration {
    Duration::from_secs(5)
}
