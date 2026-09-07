import { useEffect, useRef, useState } from 'react';

import type { MetricsFrame } from 'src/types/metrics';

/** Build the metrics WebSocket URL (same-origin in prod, port 18083 in Vite dev). */
function metricsUrl(): string {
    const isDev = window.location.port === '3039';
    const proto = window.location.protocol === 'https:' ? 'wss' : 'ws';
    const host = isDev ? `${window.location.hostname}:18083` : window.location.host;
    return `${proto}://${host}/api/v1/ws/metrics`;
}

export type MetricsConnectionStatus = 'connecting' | 'open' | 'closed';

export type UseMetricsResult = {
    /** Most recent frame, or null before the first frame arrives. */
    latest: MetricsFrame | null;
    /** Rolling window of recent frames for charting (oldest first). */
    history: MetricsFrame[];
    status: MetricsConnectionStatus;
};

/**
 * Subscribe to the broker's live-metrics WebSocket. Keeps the latest frame plus a
 * rolling history window, and auto-reconnects with backoff while mounted.
 */
export function useMetrics(historySize = 60): UseMetricsResult {
    const [latest, setLatest] = useState<MetricsFrame | null>(null);
    const [history, setHistory] = useState<MetricsFrame[]>([]);
    const [status, setStatus] = useState<MetricsConnectionStatus>('connecting');

    const wsRef = useRef<WebSocket | null>(null);
    const reconnectRef = useRef<ReturnType<typeof setTimeout> | null>(null);
    const closedByUs = useRef(false);

    useEffect(() => {
        closedByUs.current = false;
        let attempt = 0;

        const connect = () => {
            setStatus('connecting');
            const ws = new WebSocket(metricsUrl());
            wsRef.current = ws;

            ws.onopen = () => {
                attempt = 0;
                setStatus('open');
            };

            ws.onmessage = (event) => {
                try {
                    const frame = JSON.parse(event.data) as MetricsFrame;
                    setLatest(frame);
                    setHistory((prev) => {
                        const next = [...prev, frame];
                        return next.length > historySize ? next.slice(next.length - historySize) : next;
                    });
                } catch {
                    /* ignore malformed frames */
                }
            };

            ws.onclose = () => {
                setStatus('closed');
                if (closedByUs.current) return;
                attempt += 1;
                const delay = Math.min(1000 * 2 ** attempt, 15000);
                reconnectRef.current = setTimeout(connect, delay);
            };

            ws.onerror = () => {
                ws.close();
            };
        };

        connect();

        return () => {
            closedByUs.current = true;
            if (reconnectRef.current) clearTimeout(reconnectRef.current);
            wsRef.current?.close();
        };
    }, [historySize]);

    return { latest, history, status };
}
