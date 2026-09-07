import { create } from 'zustand';
import type { FlashbarProps } from '@cloudscape-design/components/flashbar';

type FlashType = FlashbarProps.Type;

type NotificationState = {
    items: FlashbarProps.MessageDefinition[];
};

type NotificationActions = {
    /** Push a flash message; returns its id. Auto-dismisses success/info after 5s. */
    notify: (type: FlashType, content: React.ReactNode, header?: string) => string;
    dismiss: (id: string) => void;
    clear: () => void;
};

let seq = 0;

export const useNotificationStore = create<NotificationState & NotificationActions>((set, get) => ({
    items: [],

    notify: (type, content, header) => {
        const id = `n-${(seq += 1)}`;
        const item: FlashbarProps.MessageDefinition = {
            id,
            type,
            header,
            content,
            dismissible: true,
            onDismiss: () => get().dismiss(id),
        };
        set((s) => ({ items: [item, ...s.items] }));

        if (type === 'success' || type === 'info') {
            setTimeout(() => get().dismiss(id), 5000);
        }
        return id;
    },

    dismiss: (id) => set((s) => ({ items: s.items.filter((i) => i.id !== id) })),
    clear: () => set({ items: [] }),
}));

/** Convenience helper usable outside React components. */
export const notify = (type: FlashType, content: React.ReactNode, header?: string) =>
    useNotificationStore.getState().notify(type, content, header);
