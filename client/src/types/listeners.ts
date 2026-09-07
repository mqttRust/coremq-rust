export type TlsConfig = {
    cert: string;
    key: string;
    ca?: string;
};

export type ListenerProtocol = 'tcp' | 'ws' | 'tls' | 'wss';

export type Listener = {
    name: string;
    protocol: string;
    host: string;
    port: number;
    tls?: TlsConfig | null;
    max_connections?: number | null;
    connections: number;
};

export type CreateListenerInput = {
    name: string;
    protocol: ListenerProtocol;
    host?: string;
    port: number;
    tls?: TlsConfig;
    max_connections?: number;
};

export const LISTENER_PROTOCOLS = [
    { value: 'tcp', label: 'TCP' },
    { value: 'ws', label: 'WebSocket' },
    { value: 'tls', label: 'TLS' },
    { value: 'wss', label: 'Secure WebSocket (WSS)' },
] as const;
