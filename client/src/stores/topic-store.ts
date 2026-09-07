import { create } from 'zustand';
import type { TopicInfo } from 'src/types/topics';
import { fetchTopics } from 'src/services/topics';

/** One entry in the "messages I published" history (kept across navigation). */
export type PublishRecord = {
    id: number;
    topic: string;
    payload: string;
    qos: number;
    retain: boolean;
    time: string;
    ok: boolean;
};

type TopicState = {
    topics: TopicInfo[];
    totalSubscriptions: number;
    publishHistory: PublishRecord[];
    loading: boolean;
    error: string | null;
};

type TopicActions = {
    fetch: () => Promise<void>;
    addPublish: (record: Omit<PublishRecord, 'id' | 'time'>) => void;
    clearPublishHistory: () => void;
    clearError: () => void;
    reset: () => void;
};

const initialState: TopicState = {
    topics: [],
    totalSubscriptions: 0,
    publishHistory: [],
    loading: false,
    error: null,
};

let seq = 0;

export const useTopicStore = create<TopicState & TopicActions>((set) => ({
    ...initialState,

    fetch: async () => {
        set({ loading: true, error: null });
        try {
            const res = await fetchTopics();
            const list = res?.data ?? [];
            const total = list.reduce((sum, t) => sum + t.subscriber_count, 0);
            set({ topics: list, totalSubscriptions: total, loading: false });
        } catch (err: any) {
            set({ error: err?.message || 'Failed to load topics', loading: false });
        }
    },

    addPublish: (record) =>
        set((s) => ({
            publishHistory: [
                { ...record, id: (seq += 1), time: new Date().toLocaleTimeString() },
                ...s.publishHistory,
            ].slice(0, 100),
        })),

    clearPublishHistory: () => set({ publishHistory: [] }),
    clearError: () => set({ error: null }),
    reset: () => set(initialState),
}));

export const selectPublishHistory = (s: TopicState & TopicActions) => s.publishHistory;

export const selectTopics = (s: TopicState & TopicActions) => s.topics;
export const selectTopicStats = (s: TopicState & TopicActions) => ({
    count: s.topics.length,
    totalSubscriptions: s.totalSubscriptions,
});
