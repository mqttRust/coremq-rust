import type { TopicInfo } from 'src/types/topics';

/** One frame pushed by the backend `/api/v1/ws/metrics` WebSocket (once per second). */
export type MetricsFrame = {
    timestamp: string;
    memory_mb: number;
    cpu_percent: number;
    client_count: number;
    topics: TopicInfo[];
};
