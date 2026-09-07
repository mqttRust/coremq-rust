/** Broker-wide cluster summary. `enabled: false` means no `cluster:` config section. */
export type ClusterStatus = {
    enabled: boolean;
    cluster: string;
    node_id: string;
    advertise_addr: string;
    incarnation: number;
    discovery: string;
    members_total: number;
    members_alive: number;
    routes_total: number;
    federation_links: number;
    is_federation_owner: boolean;
};

/**
 * Lifecycle of a peer as the failure detector sees it. Only `dead` triggers a
 * route purge; `suspect` is deliberately not actionable.
 */
export type NodeState = 'alive' | 'suspect' | 'dead' | 'left';

export type ClusterNode = {
    id: string;
    cluster: string;
    advertise_addr: string;
    api_addr: string | null;
    state: NodeState;
    incarnation: number;
    version: string;
    last_seen_secs: number;
    is_self: boolean;
    routes: number;
    dropped_messages: number;
};

/** One entry of the node-granular route table: which node wants this filter. */
export type ClusterRoute = {
    filter: string;
    node: string;
};

export type ClusterSession = {
    client_id: string;
    username: string;
    node: string;
    remote_addr: string;
    connected_port: number;
    connected_at: string;
    subscriptions: number;
};

export type FederationLinkState = 'connecting' | 'up' | 'down' | 'standby';

export type FederationLink = {
    name: string;
    endpoints: string[];
    forward: string[];
    accept: string[];
    state: FederationLinkState;
    sent: number;
    received: number;
    dropped: number;
    last_error: string | null;
};

export type CreateFederationInput = {
    name: string;
    endpoints: string[];
    forward: string[];
    accept: string[];
    secret: string;
};

export const EMPTY_CLUSTER_STATUS: ClusterStatus = {
    enabled: false,
    cluster: '',
    node_id: '',
    advertise_addr: '',
    incarnation: 0,
    discovery: '',
    members_total: 0,
    members_alive: 0,
    routes_total: 0,
    federation_links: 0,
    is_federation_owner: false,
};
