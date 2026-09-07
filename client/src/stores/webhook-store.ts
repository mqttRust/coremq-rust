import { create } from 'zustand';
import type { Webhook, WebhookInput } from 'src/types/webhooks';
import {
    fetchWebhooks,
    createWebhook,
    updateWebhook,
    deleteWebhook,
} from 'src/services/webhooks';

type WebhookState = {
    webhooks: Webhook[];
    loading: boolean;
    error: string | null;
};

type WebhookActions = {
    fetch: () => Promise<void>;
    create: (input: WebhookInput) => Promise<boolean>;
    update: (id: string, input: WebhookInput) => Promise<boolean>;
    remove: (id: string) => Promise<boolean>;
    clearError: () => void;
    reset: () => void;
};

const initialState: WebhookState = {
    webhooks: [],
    loading: false,
    error: null,
};

export const useWebhookStore = create<WebhookState & WebhookActions>((set, get) => ({
    ...initialState,

    fetch: async () => {
        set({ loading: true, error: null });
        try {
            const res = await fetchWebhooks();
            set({ webhooks: res?.data ?? [], loading: false });
        } catch (err: any) {
            set({ error: err?.message || 'Failed to load webhooks', loading: false });
        }
    },

    create: async (input: WebhookInput) => {
        try {
            await createWebhook(input);
            await get().fetch();
            return true;
        } catch (err: any) {
            set({ error: err?.response?.data?.message || err?.message || 'Failed to create webhook' });
            return false;
        }
    },

    update: async (id: string, input: WebhookInput) => {
        try {
            await updateWebhook(id, input);
            await get().fetch();
            return true;
        } catch (err: any) {
            set({ error: err?.response?.data?.message || err?.message || 'Failed to update webhook' });
            return false;
        }
    },

    remove: async (id: string) => {
        try {
            await deleteWebhook(id);
            await get().fetch();
            return true;
        } catch (err: any) {
            set({ error: err?.response?.data?.message || err?.message || 'Failed to delete webhook' });
            return false;
        }
    },

    clearError: () => set({ error: null }),
    reset: () => set(initialState),
}));

export const selectWebhooks = (s: WebhookState & WebhookActions) => s.webhooks;
