use std::collections::{BTreeSet, HashMap, HashSet};

use crate::cluster::node::NodeId;

/*
  Node-granular routing table: topic filter -> set of nodes with at least one
  local subscriber.

  Deliberately not client-granular. Node A never learns that client-42 on node B
  subscribed, only that B has *someone* interested. Replicated state is then
  proportional to (distinct filters x nodes) rather than to total client count,
  and a client connecting or disconnecting usually replicates nothing at all.
*/
#[derive(Debug, Default)]
struct RouteNode {
    children: HashMap<String, RouteNode>,
    subscribers: BTreeSet<NodeId>,
}

impl RouteNode {
    fn is_empty(&self) -> bool {
        self.children.is_empty() && self.subscribers.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct RouteTable {
    root: RouteNode,

    /* Reverse index so purging a dead node is O(its filters), not a full walk. */
    by_node: HashMap<NodeId, HashSet<String>>,

    /* Highest route epoch seen from each node, for delta-vs-snapshot decisions. */
    epochs: HashMap<NodeId, u64>,
}

impl RouteTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, node: &NodeId, filter: &str) -> bool {
        let inserted = {
            let mut current = &mut self.root;
            for level in filter.split('/') {
                current = current.children.entry(level.to_string()).or_default();
            }
            current.subscribers.insert(node.clone())
        };

        if inserted {
            self.by_node
                .entry(node.clone())
                .or_default()
                .insert(filter.to_string());
        }

        inserted
    }

    pub fn remove(&mut self, node: &NodeId, filter: &str) -> bool {
        let levels: Vec<&str> = filter.split('/').collect();
        let removed = Self::remove_recursive(&mut self.root, &levels, node);

        if removed {
            if let Some(filters) = self.by_node.get_mut(node) {
                filters.remove(filter);
                if filters.is_empty() {
                    self.by_node.remove(node);
                }
            }
        }

        removed
    }

    fn remove_recursive(node: &mut RouteNode, levels: &[&str], target: &NodeId) -> bool {
        if levels.is_empty() {
            return node.subscribers.remove(target);
        }

        let Some(child) = node.children.get_mut(levels[0]) else {
            return false;
        };

        let removed = Self::remove_recursive(child, &levels[1..], target);
        if child.is_empty() {
            node.children.remove(levels[0]);
        }

        removed
    }

    /*
      Drop every route owned by a node. Called when a node is declared Dead or
      leaves gracefully. Returns the filters that were removed.
    */
    pub fn purge_node(&mut self, node: &NodeId) -> Vec<String> {
        let Some(filters) = self.by_node.remove(node) else {
            self.epochs.remove(node);
            return Vec::new();
        };

        let filters: Vec<String> = filters.into_iter().collect();
        for filter in &filters {
            let levels: Vec<&str> = filter.split('/').collect();
            Self::remove_recursive(&mut self.root, &levels, node);
        }

        self.epochs.remove(node);
        filters
    }

    /*
      Replace a node's entire filter set. Used by RouteSnapshot, where the owner
      is authoritative and anything we hold that it no longer claims is stale.
    */
    pub fn replace_node(&mut self, node: &NodeId, filters: &[String]) {
        self.purge_node(node);
        for filter in filters {
            self.add(node, filter);
        }
    }

    /*
      Nodes that should receive a message published on `topic`.

      Returns a deduplicated set, so a node matching three different filters is
      still sent exactly one copy.
    */
    pub fn match_nodes(&self, topic: &str) -> BTreeSet<NodeId> {
        let levels: Vec<&str> = topic.split('/').collect();
        let mut out = BTreeSet::new();
        Self::match_recursive(&self.root, &levels, &mut out);
        out
    }

    fn match_recursive(node: &RouteNode, levels: &[&str], out: &mut BTreeSet<NodeId>) {
        if levels.is_empty() {
            out.extend(node.subscribers.iter().cloned());

            /*
              "sport/#" also matches the parent topic "sport" itself, per the
              MQTT spec's treatment of the multi-level wildcard.
            */
            if let Some(child) = node.children.get("#") {
                out.extend(child.subscribers.iter().cloned());
            }
            return;
        }

        if let Some(child) = node.children.get(levels[0]) {
            Self::match_recursive(child, &levels[1..], out);
        }

        if let Some(child) = node.children.get("+") {
            Self::match_recursive(child, &levels[1..], out);
        }

        if let Some(child) = node.children.get("#") {
            out.extend(child.subscribers.iter().cloned());
        }
    }

    pub fn filters_for(&self, node: &NodeId) -> Vec<String> {
        self.by_node
            .get(node)
            .map(|f| f.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn known_nodes(&self) -> Vec<NodeId> {
        self.by_node.keys().cloned().collect()
    }

    pub fn epoch_for(&self, node: &NodeId) -> u64 {
        self.epochs.get(node).copied().unwrap_or(0)
    }

    pub fn set_epoch(&mut self, node: &NodeId, epoch: u64) {
        self.epochs.insert(node.clone(), epoch);
    }

    /*
      Drop entries owned by nodes that are no longer members. Repairs drift from
      a delta that arrived after its owner was already purged.
    */
    pub fn sweep_orphans(&mut self, live: &HashSet<NodeId>) -> Vec<NodeId> {
        let orphans: Vec<NodeId> = self
            .by_node
            .keys()
            .filter(|n| !live.contains(*n))
            .cloned()
            .collect();

        for node in &orphans {
            self.purge_node(node);
        }

        orphans
    }

    pub fn total_entries(&self) -> usize {
        self.by_node.values().map(|f| f.len()).sum()
    }

    /* Flattened view for the admin API: (filter, node). */
    pub fn entries(&self) -> Vec<(String, NodeId)> {
        let mut out = Vec::new();
        for (node, filters) in &self.by_node {
            for filter in filters {
                out.push((filter.clone(), node.clone()));
            }
        }
        out.sort();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str) -> NodeId {
        NodeId::new(name)
    }

    #[test]
    fn exact_filter_matches() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "sensors/temp");

        assert_eq!(t.match_nodes("sensors/temp").len(), 1);
        assert!(t.match_nodes("sensors/humidity").is_empty());
    }

    #[test]
    fn single_level_wildcard_matches_one_level_only() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "sensors/+/temp");

        assert_eq!(t.match_nodes("sensors/room1/temp").len(), 1);
        assert!(t.match_nodes("sensors/room1/sub/temp").is_empty());
    }

    #[test]
    fn multi_level_wildcard_matches_descendants_and_parent() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "sensors/#");

        assert_eq!(t.match_nodes("sensors/a/b/c").len(), 1);
        /* The spec says "sport/#" also matches "sport". */
        assert_eq!(t.match_nodes("sensors").len(), 1);
    }

    #[test]
    fn a_node_matching_several_filters_appears_once() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "sensors/#");
        t.add(&node("a"), "sensors/+/temp");
        t.add(&node("a"), "sensors/room1/temp");

        /* One copy per node is what stops duplicate delivery. */
        assert_eq!(t.match_nodes("sensors/room1/temp").len(), 1);
    }

    #[test]
    fn several_nodes_on_one_filter_all_match() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "sensors/#");
        t.add(&node("b"), "sensors/#");

        assert_eq!(t.match_nodes("sensors/x").len(), 2);
    }

    #[test]
    fn purging_a_node_removes_only_its_routes() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "sensors/#");
        t.add(&node("b"), "sensors/#");

        let purged = t.purge_node(&node("a"));
        assert_eq!(purged, vec!["sensors/#".to_string()]);

        let matched = t.match_nodes("sensors/x");
        assert_eq!(matched.len(), 1);
        assert!(matched.contains(&node("b")));
    }

    #[test]
    fn removing_the_last_filter_prunes_the_branch() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "a/b/c/d");
        assert!(t.remove(&node("a"), "a/b/c/d"));

        assert!(t.root.children.is_empty());
        assert_eq!(t.total_entries(), 0);
    }

    #[test]
    fn replace_node_drops_filters_the_owner_no_longer_claims() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "old/#");
        t.replace_node(&node("a"), &["new/#".to_string()]);

        assert!(t.match_nodes("old/x").is_empty());
        assert_eq!(t.match_nodes("new/x").len(), 1);
    }

    #[test]
    fn orphan_sweep_drops_unknown_nodes() {
        let mut t = RouteTable::new();
        t.add(&node("a"), "x/#");
        t.add(&node("ghost"), "x/#");

        let live: HashSet<NodeId> = [node("a")].into_iter().collect();
        let orphans = t.sweep_orphans(&live);

        assert_eq!(orphans, vec![node("ghost")]);
        assert_eq!(t.match_nodes("x/y").len(), 1);
    }

    #[test]
    fn duplicate_add_is_idempotent() {
        let mut t = RouteTable::new();
        assert!(t.add(&node("a"), "x/#"));
        assert!(!t.add(&node("a"), "x/#"));
        assert_eq!(t.total_entries(), 1);
    }
}
