# Clustering, Federation and Auto-Cleaning

Design document. Status: **proposal, not implemented**. No code in `server/coremq-server/src/cluster/` exists yet.

Decisions already locked in:

- Discovery is a **pluggable trait, static seed list first**.
- Multi-cluster means **federation** — autonomous clusters bridged by topic-filtered links.
- Cluster metadata (users, listeners, webhooks, auth config) replicates by **gossip with last-write-wins**.

---

## 1. Goals

1. N broker nodes behave as one logical broker. A client subscribing on node A receives messages published on node B.
2. Nodes find each other automatically. Adding a node requires no edit to the other nodes' configs.
3. When a node dies, every trace of it disappears from the survivors without operator action.
4. Independent clusters can exchange a filtered subset of traffic across a WAN.
5. The dashboard shows the whole cluster, not just the node it happens to be talking to.

## 2. Non-goals

- **No strong consistency.** There is no consensus protocol. Concurrent admin edits on two nodes resolve last-write-wins, and the loser is silently discarded. Section 10 covers the split-brain consequences.
- **No session migration.** A client is pinned to the node it connected to. If that node dies the client reconnects and lands somewhere else with a fresh session. This is acceptable *today* only because the broker has no offline message queue — see section 12.
- **No cluster-wide QoS 1/2 guarantees** beyond what a single node offers today.
- **No shared subscriptions** (`$share/...`). Cluster-wide shared subscriptions need the route table to carry load-balancing groups; deferred.

---

## 3. What the current code gives us

| Component | File | Relevance |
|---|---|---|
| `Engine` actor | `src/engine/engine.rs:78` | Single task, owns all mutable state, driven by three channels. Every publish funnels through `Engine::publish()` (`engine.rs:46`) — the one seam clustering needs. |
| Local subscription trie | `src/services/topic.rs` | `topic → client_ids`. The cluster adds a *parallel* table `filter → node_ids`. |
| Session registry | `src/services/session.rs` | `DashMap<client_id, Session>`, each holding a local `mpsc::Sender`. Purely local; a remote client cannot be addressed. |
| Transport loops | `src/transport/tcp.rs:16`, `ws.rs` | Feed `ConnectCommand` / `PubSubCommand` into the engine. Unchanged by clustering. |
| Node-local store | `src/storage/redb/` | Four repos over one redb file. Diverges across nodes today. |
| Admin API | `src/api/router.rs` | Where the cluster endpoints attach. |

Three properties make this tractable: state lives in one actor, publishes have exactly one chokepoint, and the storage repos share a uniform upsert/get/delete/get_all shape.

---

## 4. Node identity

```rust
/* src/cluster/node.rs */

pub struct NodeId(String);

pub struct NodeDescriptor {
    pub id: NodeId,
    pub cluster: String,
    pub advertise_addr: SocketAddr,
    pub api_addr: SocketAddr,
    pub incarnation: u64,
    pub started_at: i64,
    pub version: String,
}
```

- `id` is stable across restarts. Resolution order: `cluster.node_id` in config → `COREMQ_NODE_ID` → persisted value in redb → a generated UUIDv4 that is then persisted. A node that loses its id looks like a brand-new node to everyone else, which is survivable but causes a full route resync.
- `incarnation` increments on every process start and is the tiebreaker for stale gossip. A node that restarts fast enough to still be `Alive` in a peer's table is detected by the higher incarnation, which forces the peer to drop all old state for that id.
- `cluster` gates membership. A `Hello` whose cluster name does not match is rejected in `HelloAck`. This is the guard against accidentally joining staging nodes to production.

### Config schema

```yaml
cluster:
  enabled: true
  name: prod
  node_id: ""                 # blank = generate and persist
  bind: "0.0.0.0:4370"
  advertise: ""               # blank = first non-loopback interface : bind port

  discovery:
    type: static              # static | dns | mdns | k8s
    seeds:
      - 10.0.0.1:4370
      - 10.0.0.2:4370
    interval: 10s             # rediscovery sweep

  failure_detector:
    probe_interval: 1s
    suspect_after: 3s
    dead_after: 10s

  cleanup:
    reconcile_interval: 30s
    orphan_sweep_interval: 60s
    tombstone_ttl: 24h

federation:
  - name: eu-west
    endpoints: ["mq-eu-1.example.com:4370", "mq-eu-2.example.com:4370"]
    forward: ["sensors/#", "telemetry/+/status"]
    accept:  ["cmd/eu/#"]
    tls: true
```

`cluster.enabled: false` must be a genuine no-op — no port bound, no task spawned, zero overhead on the publish path. Single-node deployments are the common case and must not regress.

---

## 5. Discovery

```rust
/* src/cluster/discovery.rs */

#[async_trait]
pub trait Discovery: Send + Sync {
    /* Addresses that may be cluster peers. May include self and dead nodes. */
    async fn discover(&self) -> anyhow::Result<Vec<SocketAddr>>;

    fn name(&self) -> &'static str;
}
```

Discovery only produces *candidate addresses*. It never decides membership — that is the failure detector's job (section 7). This split is what lets a stale DNS record or a departed seed be harmless.

A background task calls `discover()` every `discovery.interval`, diffs against known peers, and dials anything new. Dialing an address that is already connected, is self, or is dead is a no-op.

| Backend | Phase | Mechanism |
|---|---|---|
| `static` | 1 | Return the configured seed list verbatim. |
| `dns` | 3 | Resolve A/AAAA of a headless service name, or SRV for host+port. Kubernetes StatefulSets and Docker Compose service names both work. |
| `mdns` | 3 | Multicast DNS-SD `_coremq._tcp.local`. Zero-config on a LAN; does not survive most cloud VPCs. |
| `k8s` | 4 | List pods by label selector via the API server. Needed only when DNS is insufficient (e.g. cross-namespace). |

Seeds are a bootstrap hint, not a membership list. Once node C learns about A and B, C stays in the cluster even if every configured seed is removed.

---

## 6. Peer transport and wire protocol

One TCP connection per peer pair, on `cluster.bind` (default 4370), optionally TLS using the existing `transport/tls.rs` machinery. To avoid two half-open connections when both sides dial simultaneously, the pair with the **lexicographically smaller node id keeps its outbound connection** and the other side drops its own.

Framing: `[len: u32 big-endian][bincode payload]`, with a 16 MiB frame cap.

```rust
/* src/cluster/protocol.rs */

pub enum ClusterMessage {
    Hello { node: NodeDescriptor, protocol_version: u16 },
    HelloAck { node: NodeDescriptor, accepted: bool, reason: Option<String> },

    Ping { seq: u64 },
    Pong { seq: u64 },
    PingReq { target: NodeId, seq: u64 },      /* indirect probe */

    Membership { entries: Vec<MemberEntry> },

    RouteAdd { node: NodeId, filter: String, epoch: u64 },
    RouteDel { node: NodeId, filter: String, epoch: u64 },
    RouteSyncRequest { node: NodeId, since_epoch: u64 },
    RouteSnapshot { node: NodeId, filters: Vec<String>, epoch: u64 },

    Forward { origin: NodeId, packet: WirePublish, cluster_path: Vec<String> },

    SessionClaim { client_id: String, node: NodeId, claimed_at: i64 },

    MetaDelta { entries: Vec<MetaEntry> },

    Leave { node: NodeId, reason: LeaveReason },
}
```

`protocol_version` is checked in `Hello`. Mismatched versions are rejected with a reason rather than allowed to misparse. Bump it on any breaking change to this enum.

The peer inbox must be a **bounded** `mpsc::channel(N)` — `Forward` is a hot path and `AGENTS.md` reserves unbounded channels for control-plane signals. When a peer's outbound queue is full, drop the message and increment a counter; blocking the engine actor on a slow peer would stall the entire node.

---

## 7. Membership and failure detection

For the cluster sizes this targets (3–15 nodes), a full mesh with direct heartbeats plus indirect probing is enough, and it needs no new dependency — the mesh already exists to carry `Forward`. The logic sits behind a trait so `foca` (SWIM) or `chitchat` (scuttlebutt) can replace it if node counts grow past the point where O(n²) connections hurt.

State machine per peer:

```
                probe_interval
   ┌──────────────── Ping/Pong ok ◄──────────────┐
   │                                             │
Alive ──── 3 missed probes ────► Suspect ────────┘  (any Pong, or a peer
   │                                │                reporting it Alive
   │                                │                with a ≥ incarnation)
   │                          dead_after elapsed
   │                                ▼
   └──── Leave received ────────► Dead ──── purge (section 9) ───► forgotten
```

- **Suspect** is deliberately not actionable. Routes stay, forwarding continues. A 3-second network hiccup must not tear down the route table.
- Before declaring Suspect, the prober asks `k = 3` random peers to probe the target (`PingReq`). A node is only suspected if the indirect probes also fail. This is what stops one bad link between two nodes from evicting a healthy third.
- **Dead** is the only state that triggers cleanup, and it is reached either by `dead_after` expiry or immediately on a graceful `Leave`.
- Refutation: a node that sees itself marked Suspect broadcasts a `Membership` entry with an incremented incarnation, which supersedes the suspicion everywhere.

---

## 8. Routing

Two tables, deliberately separate:

| Table | Scope | Key → value | Owner |
|---|---|---|---|
| Local trie | this node | `topic → client_ids` | `TopicService` (`services/topic.rs`), unchanged |
| Route table | cluster | `filter → set<NodeId>` | `ClusterRouter` (`cluster/router.rs`), new |

The route table is *node-granular*, not client-granular. Node A never learns that `client-42` on node B subscribed to `sensors/#` — only that node B has *someone* interested. This keeps the replicated state proportional to (distinct filters × nodes) instead of total client count, and it means a client connecting or disconnecting usually replicates nothing at all.

### Subscribe

```
client SUBSCRIBE sensors/#
  → TopicService::subscribe()                        (existing)
  → if this is the FIRST local subscriber for the filter:
        epoch += 1
        broadcast RouteAdd { node: self, filter, epoch }
```

### Unsubscribe / disconnect

```
  → TopicService::unsubscribe() / remove_client()     (existing)
  → if that was the LAST local subscriber for the filter:
        epoch += 1
        broadcast RouteDel { node: self, filter, epoch }
```

This 0→1 / 1→0 edge detection is the load-bearing optimisation. It requires a change to `TopicService`, which currently returns `()` from both methods — see section 13.

### Publish

`Engine::publish()` (`engine.rs:46`) becomes:

```rust
fn publish(&self, p: PublishPacket, origin: Origin) {
    /* Webhook dispatch and local fan-out are unchanged. */
    self.deliver_local(&p);

    /*
      A message that arrived from a peer has already been fanned out
      cluster-wide by its origin node. Re-forwarding it would loop.
    */
    if origin != Origin::Local {
        return;
    }

    if let Some(cluster) = &self.cluster {
        for node in cluster.router.match_nodes(&p.topic) {
            cluster.send(node, ClusterMessage::Forward { .. });
        }
        cluster.federation.forward(&p);
    }
}
```

**Loop prevention rests on one invariant: a publish is forwarded exactly once, by the node that received it from a client.** `Origin::Remote(_)` and `Origin::Federation(_)` messages are delivered locally and go no further. `match_nodes` returns a deduplicated node set, so a node matching three different filters still receives one copy.

### Epochs and resync

Each node keeps a monotonic `epoch` over its own route set, incremented on every announced change. Peers record `last_epoch[node]`. On reconnect, a peer sends `RouteSyncRequest { since_epoch }`; if the requested epoch is older than the oldest retained delta, the owner replies with a full `RouteSnapshot` instead. This makes a missed delta self-healing rather than permanent.

---

## 9. Auto-cleaning

The explicit requirement. Every mechanism that removes state, and what triggers it:

| Trigger | Action | Timing |
|---|---|---|
| Peer TCP connection drops | Mark Suspect. **No cleanup.** Keep routes and keep forwarding. | immediate |
| Suspect exceeds `dead_after` | Mark Dead → purge every route entry owned by that node; drop cached remote session records; prune trie nodes left empty | `dead_after` (default 10s) |
| Graceful `Leave` received | Same purge, skipping the Suspect phase entirely | immediate |
| Node reappears with higher `incarnation` | Discard all state held for the old incarnation, then full `RouteSyncRequest` | on `Hello` |
| Local client disconnects | Existing `Engine::drop_client()`, plus `RouteDel` if it was the last local subscriber for a filter | immediate |
| Route entry names an unknown node | Orphan sweep drops it — repairs drift from a delta that arrived after its owner was purged | `orphan_sweep_interval` (60s) |
| Periodic reconcile | Exchange route digests with each peer; pull anything missing, drop anything the owner no longer claims | `reconcile_interval` (30s) |
| Metadata delete | Write a tombstone (key + timestamp), not a hard delete — a hard delete would be resurrected by any peer still holding the old value | immediate |
| Tombstone ages out | Hard-delete from redb | `tombstone_ttl` (24h) |
| Federation link drops | Purge routes learned over that link; local clients simply stop receiving remote traffic | immediate |
| Process shutdown | Broadcast `Leave`, flush, close. Best-effort with a 2s deadline — a `SIGKILL` degrades to the `dead_after` path | on SIGTERM |

Two rules govern the whole table:

1. **Only Dead triggers purges.** Suspect never does. Transient network trouble must not cause a route flap, because a flap means dropped messages for every client behind that node.
2. **Every delete is idempotent and re-derivable.** Anything the delta path can lose, the reconcile path restores, and anything the reconcile path misses, the orphan sweep drops. No cleanup step is the single point of correctness.

---

## 10. Metadata replication

Users, listeners, webhooks and MQTT auth config live in node-local redb (`storage/redb/`). Under gossip LWW every record gains:

```rust
pub struct MetaEntry {
    pub table: MetaTable,        /* User | Listener | Webhook | AuthConfig */
    pub key: String,
    pub value: Option<Vec<u8>>,  /* None = tombstone */
    pub updated_at: i64,         /* milliseconds */
    pub updated_by: NodeId,      /* tiebreaker for identical timestamps */
}
```

Write path: apply locally, then broadcast `MetaDelta`. Receive path: apply only if `(updated_at, updated_by) > (local_updated_at, local_updated_by)`. Ties break on node id so every node converges on the same winner regardless of arrival order.

### Consequences you are accepting

- **Concurrent edits lose data silently.** Two admins editing the same webhook on two nodes within the clock skew window: one edit vanishes with no error shown. Mitigation is a UI-level "last modified by node X at T" hint, not a fix.
- **Clock skew is correctness-relevant.** LWW compares wall-clock timestamps across machines. Without NTP a node whose clock runs 10 minutes fast wins every conflict. NTP is a **hard operational requirement** and the node should log a warning when a peer's `Hello` timestamp skews more than 5s from local time.
- **Split-brain writes both sides.** A partition leaves both halves live, both accepting admin writes. On heal, LWW merges per key — meaning the result can be a mix of both halves that never existed as a coherent config on either side. Raft for the metadata layer is the only real fix; the `Membership` trait boundary keeps that door open.

Credential hashes replicate as opaque bytes. The federation link must never carry `MetaDelta` — federated clusters share traffic, not identity.

---

## 11. Federation

Federation is deliberately *not* clustering. A federation link is a client connection into another cluster's peer port, and carries only two message kinds: `Forward`, and route announcements for allowlisted filters. No membership, no metadata, no failure detector participation.

```
   cluster: prod                          cluster: eu-west
   ┌──────────────────┐                   ┌──────────────────┐
   │ node-1 ◄──► node-2│                  │ node-a ◄──► node-b│
   │    ▲             │                   │    ▲             │
   └────┼─────────────┘                   └────┼─────────────┘
        │        forward: sensors/#            │
        └──────────────────────────────────────┘
                 accept:  cmd/eu/#
```

- **Asymmetric filters.** `forward` is what we push out; `accept` is what we ingest. Each side configures both, and a message must pass the sender's `forward` *and* the receiver's `accept` to cross. Neither cluster can unilaterally flood the other.
- **Loop prevention.** `Forward.cluster_path` accumulates cluster names. A node drops any message whose path already contains its own cluster. This makes cycles (A→B→C→A) safe, which the `origin != Local` check alone would not.
- **One elected link owner per cluster.** If every node dialled the remote cluster, a federated publish would be duplicated N times. The node with the lowest id among Alive members owns each federation link; on its death the next-lowest takes over. This is an election over gossip state, not consensus — a brief partition can produce two owners and hence duplicate deliveries. MQTT already permits duplicates, so this is an acceptable failure mode, but it must be documented rather than discovered.
- Remote routes are namespaced (`cluster:filter`) so a federated subscription can never be mistaken for a local cluster route.

---

## 12. Retained messages, wills, and offline queues

The broker implements **none of the three** today:

- **Retained messages.** The `retain` flag is decoded (`decoder.rs:163`), re-encoded on delivery (`encoder.rs:91`) and reported to webhooks (`engine.rs:52`), but nothing ever stores a retained message. A late subscriber gets nothing.
- **Will messages.** `will_topic` and `will_message` are parsed off the CONNECT packet (`decoder.rs:86-109`) and then discarded — `ConnectPacket` has no fields to hold them.
- **Offline queues.** No inflight or queued-message state exists for `clean_session: false`.

This absence is what makes "no session migration" (section 2) defensible right now: there is no per-session state worth migrating. That stops being true the moment any of the three lands, and all three are cluster-wide problems:

- A retained message must be visible to a client subscribing on *any* node, so the retained store has to be replicated. It fits the section 10 LWW model cleanly — a retained message is a value keyed by topic.
- A will must fire cluster-wide when its client drops. Worse, when a *node* dies, the wills of every client it owned must fire, published by some surviving node. That means will payloads have to be replicated at CONNECT time, not held only on the owning node, and the section 9 Dead transition has to publish them as part of the purge.
- Offline queues are per-session state that must either follow the client to its new node or be readable remotely.

None of this is in scope here. It is flagged because retrofitting these onto a cluster is considerably harder than designing them alongside it — the will case in particular changes what the Dead-node cleanup path has to do.

---

## 13. Prerequisite changes to existing code

Real work items in code that exists today, not new modules:

1. **`src/protocol/packets.rs`** — `PublishPacket`, `ConnectPacket` and `UnsubscribePacket` derive only `Debug, Clone`. Forwarding needs `Serialize, Deserialize`. Prefer a separate `WirePublish` struct over deriving on the protocol types, so the wire format is versioned independently of the in-memory representation.
2. **`src/engine/commands.rs`** — `PubSubCommand::Publish(PublishPacket)` carries no origin. Add `Origin { Local, Remote(NodeId), Federation(String) }` as a second field. Without it the loop guard in section 8 cannot be written.
3. **`src/services/topic.rs`** — `subscribe()` and `unsubscribe()` return `()`. They must report whether the call was a 0→1 or 1→0 transition for the filter, or every subscribe broadcasts a redundant `RouteAdd`.
4. **`src/models/mqtt/session.rs`** — add `node_id` so the dashboard can show which node owns each client.
5. **`src/services/session.rs`** — `get_paginated()` (`session.rs:74`) paginates one node's `DashMap`. Cluster-wide pagination needs either scatter-gather across peers or a gossiped per-node count. Scatter-gather with a deadline is simpler and stays correct as nodes come and go.
6. **`src/engine/engine.rs`** — new `AdminCommand` variants for cluster queries. Per `AGENTS.md`, **every** match arm must send on its `oneshot::Sender` including error paths, or the API handler hangs.
7. **`src/utils/config.rs`** — parse the `cluster:` and `federation:` sections; both absent must mean disabled.
8. **`src/main.rs`** — spawn the cluster runtime after the engine (`main.rs:81`) and before `axum::serve`, and hand the engine a handle to it.
9. **Channel discipline** — `main.rs:64-66` uses `unbounded_channel()` for connect and pubsub. `AGENTS.md` requires bounded channels on hot paths. The new peer channels must be bounded regardless; converting the existing ones is a separate fix.

New dependencies: none required for phase 1 and 2. `bincode`, `tokio`, `serde`, `uuid` and `tokio-rustls` are already in `Cargo.toml`. Phase 3 adds a DNS resolver (`hickory-resolver`) and an mDNS crate.

---

## 14. Admin API and dashboard

```
GET    /api/v1/cluster                 cluster name, self node, health summary
GET    /api/v1/cluster/nodes           id, addr, state, incarnation, uptime, connections, last_seen
POST   /api/v1/cluster/nodes           manual join by address
DELETE /api/v1/cluster/nodes/:id       force-remove a node stuck in Dead
GET    /api/v1/cluster/routes          route table, filterable by node or filter
GET    /api/v1/cluster/federation      link status, per-link message counters
POST   /api/v1/cluster/federation      create link
DELETE /api/v1/cluster/federation/:n   drop link
```

A new **Cluster** page in `client/src/sections/cluster/`, following the conventions in `AGENTS.md`: view logic in `sections/<feature>/<feature>_view.tsx`, a Zustand store split into `State` / `Actions` with a `reset()`, and every user-facing string in `t()` with keys present in **all three** of `client/src/118n/{en,ko,uz}.json`.

Existing pages become cluster-aware: Sessions and Topics gain a "Node" column, and their totals aggregate across the cluster rather than reporting one node's view.

---

## 15. Phased delivery

| Phase | Contents | Verifiable by |
|---|---|---|
| **1. Data path** | Node identity, static discovery, peer mesh, route table, publish forwarding, loop guard. No failure detection — a dead peer just stops receiving. | 2 nodes: subscribe on A, publish on B, message arrives |
| **2. Membership + cleanup** | Heartbeats, indirect probing, Suspect/Dead, the full section 9 cleanup matrix, reconcile and orphan sweeps, graceful leave | Kill -9 a node; routes vanish from survivors within `dead_after` |
| **3. Discovery backends** | DNS/SRV, mDNS, k8s behind the `Discovery` trait | Scale a StatefulSet from 1 to 5 with no config change |
| **4. Metadata gossip** | LWW replication of the four redb repos, tombstones + TTL, skew warnings | Create a user on A, appears on B and C |
| **5. Federation** | Cross-cluster links, directional filters, `cluster_path` loop guard, link-owner election | Two clusters, allowlisted topic crosses, non-allowlisted does not |
| **6. UI + API** | Cluster endpoints, Cluster page, node column on Sessions/Topics | Dashboard shows all nodes, force-remove works |

Phases 1 and 2 are the useful minimum — that is a working cluster with auto-cleaning. Phases 4 and 5 are independent of each other and can be reordered.

### Testing

Integration tests belong in `tests/` and run from the workspace root (`AGENTS.md`). The cluster-specific cases that matter:

- Publish forwarding, and **no duplicate delivery** when a node matches multiple filters.
- Loop guard: a `Forward` is never re-forwarded.
- Dead-node purge completes within `dead_after`.
- Route table converges after a healed partition.
- Rejoin with a higher incarnation discards stale routes.
- LWW resolves a concurrent write deterministically on both nodes.
- Federation drops a non-allowlisted topic in both directions.
- A cycle A→B→C→A terminates.

Failure injection needs a way to pause a node's peer I/O without killing the process, otherwise partition tests can only be written as `kill -9`.

---

## 16. Open questions

1. **Cluster-wide client-id uniqueness.** MQTT requires it. `SessionClaim` broadcast on CONNECT is the plan, but it is racy: two nodes can accept the same client id within one round trip and then both disconnect it, so the client is rejected from everywhere. A short claim-then-confirm handshake fixes this at the cost of connect latency on every connection. Worth it, or accept the rare double-kick?
2. **Should `Engine` own the cluster handle, or run as a peer actor?** Owning it keeps the publish path synchronous and simple. A separate actor keeps peer I/O off the engine task, which matters if a slow peer can stall the mesh. Leaning towards a separate actor with a bounded channel, and the engine holding only a sender.
3. **Authentication between nodes.** A shared cluster secret in `Hello`, mutual TLS, or nothing (assume a trusted network)? An unauthenticated peer port is a full read/write channel into the broker's routing, so "nothing" is only tenable behind a VPC boundary. mTLS reuses `transport/tls.rs`.
4. **Metrics.** `/api/v1/metrics` currently reports one node. Aggregate across the cluster, or report per-node and let the dashboard sum? Per-node is more honest and lets the UI show outliers.
5. **Backpressure on a slow peer.** Dropping `Forward` messages when the queue is full silently loses QoS 0 traffic — acceptable — but also QoS 1, which is not. Options: separate queues per QoS, or disconnect and resync a peer that falls too far behind.
