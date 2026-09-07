use dashmap::{DashMap, DashSet};
use std::sync::Arc;

use crate::models::topic_info::TopicInfo;

#[derive(Debug, Default)]
pub struct TopicNode {
    children: DashMap<String, Arc<TopicNode>>,
    subscribers: DashSet<String>,
}

#[derive(Debug, Default)]
pub struct TopicService {
    root: Arc<TopicNode>,
}

impl TopicService {
    pub fn new() -> Self {
        Self {
            root: Arc::new(TopicNode::default()),
        }
    }

    /*
      Returns true when this was the first local subscriber for the filter.

      The cluster only announces a route on that 0 -> 1 edge, so a node with a
      thousand clients on one filter replicates a single RouteAdd.
    */
    pub fn subscribe(&self, topic: &str, client_id: &str) -> bool {
        let mut current = Arc::clone(&self.root);

        for level in topic.split('/') {
            current = {
                /*
                  Ensure the child node exists for this topic level.
                */
                let node = current.children.entry(level.to_string())
                    .or_insert_with(|| Arc::new(TopicNode::default()))
                    .clone();

                node
            };
        }

        let was_empty = current.subscribers.is_empty();

        /*
          Add the client as a subscriber for this topic node.
        */
        current.subscribers.insert(client_id.to_string());

        was_empty
    }

    /*
      Returns true when that was the last local subscriber for the filter, which
      is the 1 -> 0 edge the cluster turns into a RouteDel.
    */
    pub fn unsubscribe(&self, topic: &str, client_id: &str) -> bool {
        let levels: Vec<&str> = topic.split('/').collect();
        let had = self.has_subscribers(topic);
        self.remove_recursive(&self.root, &levels, client_id);
        had && !self.has_subscribers(topic)
    }

    /*
      Whether any local client is subscribed to this exact filter. Distinct from
      match_subscribers, which resolves wildcards against a concrete topic.
    */
    pub fn has_subscribers(&self, topic: &str) -> bool {
        let mut current = Arc::clone(&self.root);

        for level in topic.split('/') {
            let Some(child) = current.children.get(level).map(|c| c.clone()) else {
                return false;
            };
            current = child;
        }

        !current.subscribers.is_empty()
    }

    /*
      Filters this client was the last subscriber for, so the caller can announce
      the matching RouteDels after a disconnect.
    */
    pub fn filters_orphaned_by(&self, client_id: &str) -> Vec<String> {
        let mut out = Vec::new();
        Self::orphan_recursive(&self.root, String::new(), client_id, &mut out);
        out
    }

    fn orphan_recursive(
        node: &Arc<TopicNode>,
        path: String,
        client_id: &str,
        out: &mut Vec<String>,
    ) {
        if node.subscribers.len() == 1 && node.subscribers.contains(client_id) && !path.is_empty() {
            out.push(path.clone());
        }

        for entry in node.children.iter() {
            let child_path = if path.is_empty() {
                entry.key().clone()
            } else {
                format!("{}/{}", path, entry.key())
            };
            Self::orphan_recursive(entry.value(), child_path, client_id, out);
        }
    }

    pub fn match_subscribers(&self, topic: &str) -> Vec<String> {
        let levels: Vec<&str> = topic.split('/').collect();
        let mut result = Vec::new();
        self.match_recursive(&self.root, &levels, &mut result);
        result
    }

    pub fn remove_client(&self, client_id: &str) {
        self.remove_client_recursive(&self.root, client_id);
    }

    fn remove_recursive(&self, node: &Arc<TopicNode>, levels: &[&str], client_id: &str) -> bool {
        if levels.is_empty() {
            node.subscribers.remove(client_id);
        } else if let Some(child) = node.children.get(levels[0]) {
            let should_delete = self.remove_recursive(&child, &levels[1..], client_id);
            if should_delete {
                node.children.remove(levels[0]);
            }
        }

        node.children.is_empty() && node.subscribers.is_empty()
    }

    fn match_recursive(&self, node: &Arc<TopicNode>, levels: &[&str], result: &mut Vec<String>) {
        if levels.is_empty() {
            result.extend(node.subscribers.iter().map(|r| r.clone()));
            return;
        }

        let level = levels[0];

        if let Some(child) = node.children.get(level) {
            self.match_recursive(&child, &levels[1..], result);
        }

        if let Some(child) = node.children.get("+") {
            self.match_recursive(&child, &levels[1..], result);
        }

        if let Some(child) = node.children.get("#") {
            result.extend(child.subscribers.iter().map(|r| r.clone()));
        }
    }

    /*
      Collect active topics with subscriber counts.
    */
    pub fn collect_topics(&self) -> Vec<TopicInfo> {
        let mut result = Vec::new();
        self.collect_recursive(&self.root, String::new(), &mut result);
        result
    }

    /*
      Walk the topic tree and accumulate active topics.
    */
    fn collect_recursive(&self, node: &Arc<TopicNode>, path: String, result: &mut Vec<TopicInfo>) {
        let count = node.subscribers.len();
        if count > 0 {
            result.push(TopicInfo {
                topic: path.clone(),
                subscriber_count: count,
            });
        }

        for entry in node.children.iter() {
            let child_key = entry.key().clone();
            let child_node = entry.value().clone();

            let child_path = if path.is_empty() {
                child_key
            } else {
                format!("{}/{}", path, child_key)
            };

            self.collect_recursive(&child_node, child_path, result);
        }
    }

    fn remove_client_recursive(&self, node: &Arc<TopicNode>, client_id: &str) -> bool {
        node.subscribers.remove(client_id);

        let mut empty_children = Vec::new();

        for r in node.children.iter() {
            if self.remove_client_recursive(r.value(), client_id) {
                empty_children.push(r.key().clone());
            }
        }

        for key in empty_children {
            node.children.remove(&key);
        }

        node.subscribers.is_empty() && node.children.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_first_subscriber_reports_a_transition() {
        let t = TopicService::new();

        assert!(t.subscribe("sensors/#", "c1"), "first subscriber is a 0 -> 1 edge");
        assert!(!t.subscribe("sensors/#", "c2"), "second subscriber is not");
        assert!(!t.subscribe("sensors/#", "c3"));
    }

    #[test]
    fn only_the_last_unsubscribe_reports_a_transition() {
        let t = TopicService::new();
        t.subscribe("sensors/#", "c1");
        t.subscribe("sensors/#", "c2");

        assert!(!t.unsubscribe("sensors/#", "c1"), "one subscriber remains");
        assert!(t.unsubscribe("sensors/#", "c2"), "that was the last one");
    }

    #[test]
    fn unsubscribing_something_never_subscribed_is_not_a_transition() {
        let t = TopicService::new();
        assert!(!t.unsubscribe("nothing/here", "c1"));
    }

    #[test]
    fn has_subscribers_is_exact_not_wildcard_matching() {
        let t = TopicService::new();
        t.subscribe("sensors/#", "c1");

        assert!(t.has_subscribers("sensors/#"));
        /* The concrete topic has no direct subscriber, only a matching filter. */
        assert!(!t.has_subscribers("sensors/temp"));
        assert_eq!(t.match_subscribers("sensors/temp").len(), 1);
    }

    #[test]
    fn orphaned_filters_are_reported_for_a_departing_client() {
        let t = TopicService::new();
        t.subscribe("a/b", "c1");
        t.subscribe("shared/#", "c1");
        t.subscribe("shared/#", "c2");

        let orphaned = t.filters_orphaned_by("c1");

        assert!(orphaned.contains(&"a/b".to_string()));
        assert!(
            !orphaned.contains(&"shared/#".to_string()),
            "c2 still holds that filter"
        );
    }
}