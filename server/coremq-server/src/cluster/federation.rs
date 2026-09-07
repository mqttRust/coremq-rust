use std::collections::HashMap;
use std::time::Instant;

use serde::Serialize;
use tokio::sync::mpsc;

use crate::cluster::node::NodeId;
use crate::cluster::protocol::ClusterMessage;
use crate::models::config::cluster::FederationConfig;

/*
  Match an MQTT topic against a filter, with + and # wildcards.

  Shared with the federation allowlists rather than the route trie because the
  allowlist is a short linear list, not something worth indexing.
*/
pub fn topic_matches(filter: &str, topic: &str) -> bool {
    let f: Vec<&str> = filter.split('/').collect();
    let t: Vec<&str> = topic.split('/').collect();

    let mut fi = 0;
    let mut ti = 0;

    while fi < f.len() {
        match f[fi] {
            "#" => {
                /* '#' must be the final level and matches the rest, including none. */
                return fi == f.len() - 1;
            }
            "+" => {
                if ti >= t.len() {
                    return false;
                }
                fi += 1;
                ti += 1;
            }
            level => {
                if ti >= t.len() || t[ti] != level {
                    return false;
                }
                fi += 1;
                ti += 1;
            }
        }
    }

    ti == t.len()
}

pub fn any_matches(filters: &[String], topic: &str) -> bool {
    filters.iter().any(|f| topic_matches(f, topic))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkState {
    Connecting,
    Up,
    Down,
    /* Another node in this cluster owns the link. */
    Standby,
}

pub struct FederationLink {
    pub config: FederationConfig,
    pub state: LinkState,
    pub tx: Option<mpsc::Sender<ClusterMessage>>,
    pub last_change: Instant,
    pub sent: u64,
    pub received: u64,
    pub dropped: u64,
    pub last_error: Option<String>,
    /*
      Filters the remote cluster said it will accept, learned in the handshake.
      Intersected with our forward list so neither side can flood the other.
    */
    pub remote_accept: Vec<String>,
}

impl FederationLink {
    pub fn new(config: FederationConfig) -> Self {
        Self {
            config,
            state: LinkState::Down,
            tx: None,
            last_change: Instant::now(),
            sent: 0,
            received: 0,
            dropped: 0,
            last_error: None,
            remote_accept: Vec::new(),
        }
    }

    pub fn set_state(&mut self, state: LinkState) {
        if self.state != state {
            self.state = state;
            self.last_change = Instant::now();
        }
    }

    /*
      Should a locally-published topic cross this link?

      Both our forward list and (once known) the remote's accept list must pass.
      Configuring only one side must not be enough to push traffic.
    */
    pub fn should_forward(&self, topic: &str) -> bool {
        if !any_matches(&self.config.forward, topic) {
            return false;
        }

        if self.remote_accept.is_empty() {
            return true;
        }

        any_matches(&self.remote_accept, topic)
    }

    pub fn should_accept(&self, topic: &str) -> bool {
        any_matches(&self.config.accept, topic)
    }
}

pub struct Federation {
    links: HashMap<String, FederationLink>,
    cluster_name: String,
}

impl Federation {
    pub fn new(cluster_name: String, configs: &[FederationConfig]) -> Self {
        let links = configs
            .iter()
            .map(|c| (c.name.clone(), FederationLink::new(c.clone())))
            .collect();

        Self {
            links,
            cluster_name,
        }
    }

    pub fn cluster_name(&self) -> &str {
        &self.cluster_name
    }

    pub fn get(&self, name: &str) -> Option<&FederationLink> {
        self.links.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut FederationLink> {
        self.links.get_mut(name)
    }

    pub fn all(&self) -> impl Iterator<Item = &FederationLink> {
        self.links.values()
    }

    pub fn names(&self) -> Vec<String> {
        self.links.keys().cloned().collect()
    }

    pub fn add(&mut self, config: FederationConfig) -> bool {
        if self.links.contains_key(&config.name) {
            return false;
        }
        self.links.insert(config.name.clone(), FederationLink::new(config));
        true
    }

    pub fn remove(&mut self, name: &str) -> Option<FederationLink> {
        self.links.remove(name)
    }

    /*
      A message may cross into this cluster only if the path does not already
      contain us. This is what makes a cycle (A -> B -> C -> A) terminate; the
      origin check alone would not, because each hop has a different origin.
    */
    pub fn would_loop(&self, cluster_path: &[String]) -> bool {
        cluster_path.iter().any(|c| c == &self.cluster_name)
    }

    /*
      Whether this node owns the federation links for the cluster.

      Every node dialling the remote cluster would duplicate each federated
      publish N times, so the lowest-id live node owns them. This is an election
      over gossip state, not consensus: a partition can briefly produce two
      owners and therefore duplicate deliveries.
    */
    pub fn is_owner(self_id: &NodeId, live: &[NodeId]) -> bool {
        live.iter().filter(|id| *id < self_id).count() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_wildcard_matching() {
        assert!(topic_matches("a/b", "a/b"));
        assert!(!topic_matches("a/b", "a/c"));

        assert!(topic_matches("a/+/c", "a/b/c"));
        assert!(!topic_matches("a/+/c", "a/b/d"));
        assert!(!topic_matches("a/+/c", "a/b/x/c"));

        assert!(topic_matches("a/#", "a/b/c/d"));
        /* '#' also matches the parent level itself. */
        assert!(topic_matches("a/#", "a"));
        assert!(!topic_matches("a/#", "b/c"));
    }

    #[test]
    fn a_filter_longer_than_the_topic_does_not_match() {
        assert!(!topic_matches("a/b/c", "a/b"));
        assert!(!topic_matches("a/+", "a"));
    }

    fn link(forward: &[&str], accept: &[&str]) -> FederationLink {
        FederationLink::new(FederationConfig {
            name: "eu".into(),
            endpoints: vec![],
            forward: forward.iter().map(|s| s.to_string()).collect(),
            accept: accept.iter().map(|s| s.to_string()).collect(),
            secret: String::new(),
            retry_interval: std::time::Duration::from_secs(5),
        })
    }

    #[test]
    fn forwarding_requires_the_topic_to_be_allowlisted() {
        let l = link(&["sensors/#"], &[]);

        assert!(l.should_forward("sensors/temp"));
        assert!(!l.should_forward("secrets/keys"));
    }

    #[test]
    fn the_remote_accept_list_can_veto_our_forward_list() {
        let mut l = link(&["sensors/#"], &[]);
        l.remote_accept = vec!["sensors/public/#".to_string()];

        assert!(l.should_forward("sensors/public/temp"));
        /* We would forward it, but the remote will not take it. */
        assert!(!l.should_forward("sensors/private/temp"));
    }

    #[test]
    fn accepting_is_independent_of_forwarding() {
        let l = link(&["sensors/#"], &["cmd/eu/#"]);

        assert!(l.should_accept("cmd/eu/reboot"));
        assert!(!l.should_accept("sensors/temp"));
    }

    #[test]
    fn a_message_that_already_visited_us_is_dropped() {
        let f = Federation::new("prod".into(), &[]);

        assert!(f.would_loop(&["eu".into(), "prod".into()]));
        assert!(!f.would_loop(&["eu".into(), "apac".into()]));
    }

    #[test]
    fn only_the_lowest_id_node_owns_the_links() {
        let a = NodeId::new("node-a");
        let b = NodeId::new("node-b");
        let c = NodeId::new("node-c");

        let live = vec![b.clone(), c.clone()];
        assert!(Federation::is_owner(&a, &live));

        let live = vec![a.clone(), c.clone()];
        assert!(!Federation::is_owner(&b, &live));
    }

    #[test]
    fn a_lone_node_owns_its_links() {
        assert!(Federation::is_owner(&NodeId::new("only"), &[]));
    }
}
