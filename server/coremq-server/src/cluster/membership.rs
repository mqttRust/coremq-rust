use std::collections::HashMap;
use std::time::Instant;

use serde::Serialize;

use crate::cluster::node::{NodeDescriptor, NodeId};
use crate::cluster::protocol::{MemberEntry, WireMemberState};
use crate::models::config::cluster::FailureDetectorConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberState {
    Alive,
    Suspect,
    Dead,
    Left,
}

impl MemberState {
    pub fn to_wire(self) -> WireMemberState {
        match self {
            MemberState::Alive => WireMemberState::Alive,
            MemberState::Suspect => WireMemberState::Suspect,
            MemberState::Dead => WireMemberState::Dead,
            MemberState::Left => WireMemberState::Left,
        }
    }

    pub fn from_wire(w: WireMemberState) -> Self {
        match w {
            WireMemberState::Alive => MemberState::Alive,
            WireMemberState::Suspect => MemberState::Suspect,
            WireMemberState::Dead => MemberState::Dead,
            WireMemberState::Left => MemberState::Left,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MemberState::Alive => "alive",
            MemberState::Suspect => "suspect",
            MemberState::Dead => "dead",
            MemberState::Left => "left",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Member {
    pub desc: NodeDescriptor,
    pub state: MemberState,
    pub last_seen: Instant,
    pub state_since: Instant,
    /* Set when this node was declared Dead and its state has been purged. */
    pub purged: bool,
}

/*
  Membership table plus failure detector.

  Deliberately state-only: it decides *what* changed and returns the transitions,
  leaving the caller to act on them. That keeps the cleanup policy in one place
  (the runtime) instead of spread through the detector.
*/
pub struct Membership {
    self_id: NodeId,
    members: HashMap<NodeId, Member>,
    config: FailureDetectorConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    Joined(NodeId),
    Suspected(NodeId),
    Died(NodeId),
    Recovered(NodeId),
    /* A node came back with a higher incarnation: all state for it is stale. */
    Restarted(NodeId),
}

impl Membership {
    pub fn new(self_id: NodeId, config: FailureDetectorConfig) -> Self {
        Self {
            self_id,
            members: HashMap::new(),
            config,
        }
    }

    pub fn get(&self, id: &NodeId) -> Option<&Member> {
        self.members.get(id)
    }

    pub fn all(&self) -> impl Iterator<Item = &Member> {
        self.members.values()
    }

    pub fn alive_ids(&self) -> Vec<NodeId> {
        self.members
            .values()
            .filter(|m| matches!(m.state, MemberState::Alive | MemberState::Suspect))
            .map(|m| m.desc.id.clone())
            .collect()
    }

    /*
      Ids considered live for the purposes of route ownership. Suspect nodes are
      included on purpose: suspicion must never trigger a purge, or a three
      second network hiccup drops every route behind that node.
    */
    pub fn live_ids(&self) -> std::collections::HashSet<NodeId> {
        let mut set: std::collections::HashSet<NodeId> = self
            .members
            .values()
            .filter(|m| matches!(m.state, MemberState::Alive | MemberState::Suspect))
            .map(|m| m.desc.id.clone())
            .collect();
        set.insert(self.self_id.clone());
        set
    }

    /*
      Record a successful contact. Returns a transition when this observation
      changes the node's state.
    */
    pub fn observe(&mut self, desc: NodeDescriptor) -> Option<Transition> {
        let now = Instant::now();
        let id = desc.id.clone();

        match self.members.get_mut(&id) {
            None => {
                self.members.insert(
                    id.clone(),
                    Member {
                        desc,
                        state: MemberState::Alive,
                        last_seen: now,
                        state_since: now,
                        purged: false,
                    },
                );
                Some(Transition::Joined(id))
            }
            Some(existing) => {
                /*
                  A higher incarnation means the process restarted. Everything we
                  hold for the old incarnation — routes above all — is stale and
                  must be dropped before the new one is trusted.
                */
                if desc.incarnation > existing.desc.incarnation {
                    existing.desc = desc;
                    existing.state = MemberState::Alive;
                    existing.last_seen = now;
                    existing.state_since = now;
                    existing.purged = false;
                    return Some(Transition::Restarted(id));
                }

                if desc.incarnation < existing.desc.incarnation {
                    /* Stale gossip about an older incarnation. Ignore it. */
                    return None;
                }

                existing.last_seen = now;
                let was_down = matches!(existing.state, MemberState::Suspect | MemberState::Dead);
                if was_down {
                    existing.state = MemberState::Alive;
                    existing.state_since = now;
                    existing.purged = false;
                    return Some(Transition::Recovered(id));
                }

                None
            }
        }
    }

    pub fn touch(&mut self, id: &NodeId) -> Option<Transition> {
        let now = Instant::now();
        let member = self.members.get_mut(id)?;
        member.last_seen = now;

        if matches!(member.state, MemberState::Suspect | MemberState::Dead) {
            member.state = MemberState::Alive;
            member.state_since = now;
            member.purged = false;
            return Some(Transition::Recovered(id.clone()));
        }

        None
    }

    /*
      Advance the state machine. Called on every probe tick.

      Alive -> Suspect after suspect_after with no contact.
      Suspect -> Dead after dead_after with no contact.
      Only the Dead transition is actionable; Suspect deliberately is not.
    */
    pub fn tick(&mut self) -> Vec<Transition> {
        let now = Instant::now();
        let mut transitions = Vec::new();

        for member in self.members.values_mut() {
            if matches!(member.state, MemberState::Dead | MemberState::Left) {
                continue;
            }

            let idle = now.duration_since(member.last_seen);

            match member.state {
                MemberState::Alive if idle >= self.config.suspect_after => {
                    member.state = MemberState::Suspect;
                    member.state_since = now;
                    transitions.push(Transition::Suspected(member.desc.id.clone()));
                }
                MemberState::Suspect if idle >= self.config.dead_after => {
                    member.state = MemberState::Dead;
                    member.state_since = now;
                    transitions.push(Transition::Died(member.desc.id.clone()));
                }
                _ => {}
            }
        }

        transitions
    }

    /* A graceful Leave skips Suspect entirely. */
    pub fn mark_left(&mut self, id: &NodeId) -> Option<Transition> {
        let member = self.members.get_mut(id)?;
        if matches!(member.state, MemberState::Left) {
            return None;
        }
        member.state = MemberState::Left;
        member.state_since = Instant::now();
        Some(Transition::Died(id.clone()))
    }

    pub fn mark_purged(&mut self, id: &NodeId) {
        if let Some(member) = self.members.get_mut(id) {
            member.purged = true;
        }
    }

    pub fn forget(&mut self, id: &NodeId) -> bool {
        self.members.remove(id).is_some()
    }

    /*
      Merge a peer's view of the cluster. Only Alive reports are adopted: taking
      another node's Suspect/Dead verdict at face value would let one node with a
      bad link evict a healthy third for everyone.
    */
    pub fn merge(&mut self, entries: Vec<MemberEntry>) -> Vec<Transition> {
        let mut transitions = Vec::new();

        for entry in entries {
            if entry.desc.id == self.self_id {
                continue;
            }

            match MemberState::from_wire(entry.state) {
                MemberState::Alive => {
                    if let Some(t) = self.observe(entry.desc) {
                        transitions.push(t);
                    }
                }
                MemberState::Left => {
                    /*
                      A Left report is trustworthy in a way Suspect is not: it is
                      only ever produced by the departing node itself.
                    */
                    if self.members.contains_key(&entry.desc.id) {
                        if let Some(t) = self.mark_left(&entry.desc.id) {
                            transitions.push(t);
                        }
                    }
                }
                MemberState::Suspect | MemberState::Dead => {
                    /*
                      Learn the node exists, but form our own opinion about its
                      health from our own probes.
                    */
                    if !self.members.contains_key(&entry.desc.id) {
                        if let Some(t) = self.observe(entry.desc) {
                            transitions.push(t);
                        }
                    }
                }
            }
        }

        transitions
    }

    pub fn to_entries(&self, self_desc: &NodeDescriptor) -> Vec<MemberEntry> {
        let mut entries = vec![MemberEntry {
            desc: self_desc.clone(),
            state: WireMemberState::Alive,
            incarnation: self_desc.incarnation,
        }];

        entries.extend(self.members.values().map(|m| MemberEntry {
            desc: m.desc.clone(),
            state: m.state.to_wire(),
            incarnation: m.desc.incarnation,
        }));

        entries
    }

    /*
      Nodes that have gone quiet long enough to warrant an indirect probe, but
      are not yet suspected.
    */
    pub fn needs_indirect_probe(&self) -> Vec<NodeId> {
        let now = Instant::now();
        /* Probe once we are two thirds of the way to suspecting them. */
        let threshold = self.config.suspect_after.mul_f32(0.66);

        self.members
            .values()
            .filter(|m| m.state == MemberState::Alive)
            .filter(|m| now.duration_since(m.last_seen) >= threshold)
            .map(|m| m.desc.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;

    fn desc(id: &str, incarnation: u64) -> NodeDescriptor {
        NodeDescriptor {
            id: NodeId::new(id),
            cluster: "test".into(),
            advertise_addr: "127.0.0.1:4370".parse::<SocketAddr>().unwrap(),
            api_addr: None,
            incarnation,
            started_at: 0,
            version: "0".into(),
        }
    }

    fn fast_detector() -> FailureDetectorConfig {
        FailureDetectorConfig {
            probe_interval: Duration::from_millis(1),
            suspect_after: Duration::from_millis(10),
            dead_after: Duration::from_millis(20),
            indirect_probes: 0,
        }
    }

    #[test]
    fn first_sighting_joins() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        assert_eq!(m.observe(desc("a", 1)), Some(Transition::Joined(NodeId::new("a"))));
        /* A second sighting of the same incarnation is not a transition. */
        assert_eq!(m.observe(desc("a", 1)), None);
    }

    #[test]
    fn silence_escalates_alive_to_suspect_to_dead() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        m.observe(desc("a", 1));

        std::thread::sleep(Duration::from_millis(12));
        assert_eq!(m.tick(), vec![Transition::Suspected(NodeId::new("a"))]);

        std::thread::sleep(Duration::from_millis(12));
        assert_eq!(m.tick(), vec![Transition::Died(NodeId::new("a"))]);
    }

    #[test]
    fn contact_during_suspect_recovers_without_purging() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        m.observe(desc("a", 1));

        std::thread::sleep(Duration::from_millis(12));
        m.tick();
        assert_eq!(m.get(&NodeId::new("a")).unwrap().state, MemberState::Suspect);

        assert_eq!(m.touch(&NodeId::new("a")), Some(Transition::Recovered(NodeId::new("a"))));
        assert_eq!(m.get(&NodeId::new("a")).unwrap().state, MemberState::Alive);
    }

    #[test]
    fn suspect_nodes_still_own_their_routes() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        m.observe(desc("a", 1));
        std::thread::sleep(Duration::from_millis(12));
        m.tick();

        /* Suspect must remain live, or a transient hiccup flaps the route table. */
        assert!(m.live_ids().contains(&NodeId::new("a")));
    }

    #[test]
    fn higher_incarnation_signals_a_restart() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        m.observe(desc("a", 1));

        assert_eq!(
            m.observe(desc("a", 2)),
            Some(Transition::Restarted(NodeId::new("a")))
        );
    }

    #[test]
    fn stale_gossip_about_an_older_incarnation_is_ignored() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        m.observe(desc("a", 5));
        assert_eq!(m.observe(desc("a", 3)), None);
        assert_eq!(m.get(&NodeId::new("a")).unwrap().desc.incarnation, 5);
    }

    #[test]
    fn a_peers_suspicion_does_not_evict_for_us() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        m.observe(desc("a", 1));

        m.merge(vec![MemberEntry {
            desc: desc("a", 1),
            state: WireMemberState::Suspect,
            incarnation: 1,
        }]);

        /* We keep our own opinion, formed from our own probes. */
        assert_eq!(m.get(&NodeId::new("a")).unwrap().state, MemberState::Alive);
    }

    #[test]
    fn merge_learns_unknown_nodes() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        let transitions = m.merge(vec![MemberEntry {
            desc: desc("b", 1),
            state: WireMemberState::Alive,
            incarnation: 1,
        }]);

        assert_eq!(transitions, vec![Transition::Joined(NodeId::new("b"))]);
    }

    #[test]
    fn merge_never_adds_self() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        let transitions = m.merge(vec![MemberEntry {
            desc: desc("self", 1),
            state: WireMemberState::Alive,
            incarnation: 1,
        }]);

        assert!(transitions.is_empty());
        assert!(m.get(&NodeId::new("self")).is_none());
    }

    #[test]
    fn graceful_leave_skips_suspect() {
        let mut m = Membership::new(NodeId::new("self"), fast_detector());
        m.observe(desc("a", 1));

        assert_eq!(m.mark_left(&NodeId::new("a")), Some(Transition::Died(NodeId::new("a"))));
        assert!(!m.live_ids().contains(&NodeId::new("a")));
    }
}
