import { create } from 'zustand';

import type {
    ClusterNode,
    ClusterRoute,
    ClusterStatus,
    CreateFederationInput,
    FederationLink,
} from 'src/types/cluster';
import { EMPTY_CLUSTER_STATUS } from 'src/types/cluster';
import {
    createFederation,
    deleteFederation,
    evictNode,
    getClusterStatus,
    joinNode,
    listFederation,
    listNodes,
    listRoutes,
} from 'src/services/cluster';
import { notify } from 'src/stores/notification-store';

type ClusterState = {
    status: ClusterStatus;
    nodes: ClusterNode[];
    routes: ClusterRoute[];
    links: FederationLink[];
    loading: boolean;
    /** False until the first fetch resolves, so the UI can avoid a false "disabled". */
    loaded: boolean;
};

type ClusterActions = {
    fetch: () => Promise<void>;
    join: (address: string) => Promise<boolean>;
    evict: (id: string) => Promise<boolean>;
    addLink: (input: CreateFederationInput) => Promise<boolean>;
    removeLink: (name: string) => Promise<boolean>;
    reset: () => void;
};

const initialState: ClusterState = {
    status: EMPTY_CLUSTER_STATUS,
    nodes: [],
    routes: [],
    links: [],
    loading: false,
    loaded: false,
};

const message = (err: any, fallback: string) =>
    err?.response?.data?.message || err?.message || fallback;

export const useClusterStore = create<ClusterState & ClusterActions>((set, get) => ({
    ...initialState,

    fetch: async () => {
        set({ loading: true });
        try {
            const status = (await getClusterStatus()).data ?? EMPTY_CLUSTER_STATUS;

            /*
              With clustering off the other endpoints return 404, so stop here
              rather than firing three requests that are guaranteed to fail.
            */
            if (!status.enabled) {
                set({ ...initialState, status, loaded: true });
                return;
            }

            const [nodes, routes, links] = await Promise.all([
                listNodes(),
                listRoutes(),
                listFederation(),
            ]);

            set({
                status,
                nodes: nodes.data ?? [],
                routes: routes.data ?? [],
                links: links.data ?? [],
                loading: false,
                loaded: true,
            });
        } catch (err: any) {
            notify('error', message(err, 'Failed to load cluster state'));
            set({ loading: false, loaded: true });
        }
    },

    join: async (address: string) => {
        try {
            const res = await joinNode(address);
            if (res.status_code >= 400) {
                notify('error', res.message);
                return false;
            }
            notify('success', `Dialling ${address}`);
            await get().fetch();
            return true;
        } catch (err: any) {
            notify('error', message(err, 'Failed to join node'));
            return false;
        }
    },

    evict: async (id: string) => {
        try {
            await evictNode(id);
            notify('success', `Node ${id} evicted`);
            await get().fetch();
            return true;
        } catch (err: any) {
            notify('error', message(err, 'Failed to evict node'));
            return false;
        }
    },

    addLink: async (input: CreateFederationInput) => {
        try {
            const res = await createFederation(input);
            if (res.status_code >= 400) {
                notify('error', res.message);
                return false;
            }
            notify('success', `Federation link ${input.name} created`);
            await get().fetch();
            return true;
        } catch (err: any) {
            notify('error', message(err, 'Failed to create federation link'));
            return false;
        }
    },

    removeLink: async (name: string) => {
        try {
            await deleteFederation(name);
            notify('success', `Federation link ${name} removed`);
            await get().fetch();
            return true;
        } catch (err: any) {
            notify('error', message(err, 'Failed to remove federation link'));
            return false;
        }
    },

    reset: () => set(initialState),
}));
