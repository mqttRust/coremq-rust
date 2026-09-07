import { create } from 'zustand';
import type { Listener, CreateListenerInput } from 'src/types/listeners';
import { fetchListeners, stopListener, createListener, updateListener } from 'src/services/listeners';

type ListenerState = {
    listeners: Listener[];
    loading: boolean;
    error: string | null;
};

type ListenerActions = {
    fetch: () => Promise<void>;
    create: (input: CreateListenerInput) => Promise<boolean>;
    update: (port: number, input: CreateListenerInput) => Promise<boolean>;
    stop: (port: number) => Promise<void>;
    clearError: () => void;
    reset: () => void;
};

const initialState: ListenerState = {
    listeners: [],
    loading: false,
    error: null,
};

export const useListenerStore = create<ListenerState & ListenerActions>((set, get) => ({
    ...initialState,

    fetch: async () => {
        set({ loading: true, error: null });
        try {
            const res = await fetchListeners();
            set({ listeners: Array.isArray(res) ? res : [], loading: false });
        } catch (err: any) {
            set({ error: err?.message || 'Failed to load listeners', loading: false });
        }
    },

    create: async (input: CreateListenerInput) => {
        set({ error: null });
        try {
            await createListener(input);
            await get().fetch();
            return true;
        } catch (err: any) {
            set({ error: err?.response?.data?.message || err?.message || 'Failed to create listener' });
            return false;
        }
    },

    update: async (port: number, input: CreateListenerInput) => {
        set({ error: null });
        try {
            await updateListener(port, input);
            await get().fetch();
            return true;
        } catch (err: any) {
            set({ error: err?.response?.data?.message || err?.message || 'Failed to update listener' });
            return false;
        }
    },

    stop: async (port: number) => {
        try {
            await stopListener(port);
            await get().fetch();
        } catch (err: any) {
            set({ error: err?.message || 'Failed to stop listener' });
        }
    },

    clearError: () => set({ error: null }),
    reset: () => set(initialState),
}));

export const selectListeners = (s: ListenerState & ListenerActions) => s.listeners;
