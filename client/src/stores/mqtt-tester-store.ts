import { create } from 'zustand';
import mqtt, { MqttClient, IClientOptions } from 'mqtt';

import { notify } from 'src/stores/notification-store';
import i18n from 'src/118n/index';

export type QoS = 0 | 1 | 2;
export type LogItem = { time: string; topic: string; payload: string; qos: QoS };

/**
 * The live MQTT client is kept in a module-scoped variable (NOT React state),
 * so the connection survives navigating away from and back to the page.
 */
let client: MqttClient | null = null;

/**
 * Set once per connection attempt, so a failing broker produces a single message
 * instead of one per reconnect tick.
 */
let errorNotified = false;

/** mqtt.js sometimes emits bare/empty Errors for transport failures. */
function describeError(err: unknown): string {
    const raw = err instanceof Error ? err.message : String(err ?? '');
    return raw.trim() || 'Connection refused by the broker';
}

type MqttTesterState = {
    // connection form
    url: string;
    port: string;
    path: string;
    username: string;
    password: string;
    clientId: string;
    protocol: 'ws' | 'wss';
    // status
    connected: boolean;
    connecting: boolean;
    lastError: string | null;
    // subscribe panel
    subTopics: { topic: string; qos: QoS }[];
    newSubTopic: string;
    newSubQoS: QoS;
    // publish panel
    pubTopic: string;
    pubMsg: string;
    pubQoS: QoS;
    // consoles
    subLogs: LogItem[];
    pubLogs: LogItem[];
};

type MqttTesterActions = {
    setField: (patch: Partial<MqttTesterState>) => void;
    connect: () => void;
    disconnect: () => void;
    publish: () => void;
    addSub: () => void;
    removeSub: (topic: string) => void;
    clearPubLogs: () => void;
    clearSubLogs: () => void;
};

const now = () => new Date().toLocaleTimeString();

const initialState: MqttTesterState = {
    url: 'localhost',
    port: '8083',
    path: '/mqtt',
    username: '',
    password: '',
    clientId: `mqttjs_${Math.random().toString(16).slice(2, 10)}`,
    protocol: 'ws',
    connected: false,
    connecting: false,
    lastError: null,
    subTopics: [],
    newSubTopic: '',
    newSubQoS: 0,
    pubTopic: 'test/topic',
    pubMsg: '',
    pubQoS: 0,
    subLogs: [],
    pubLogs: [],
};

export const useMqttTesterStore = create<MqttTesterState & MqttTesterActions>((set, get) => ({
    ...initialState,

    setField: (patch) => set(patch),

    connect: () => {
        if (client && client.connected) return;
        const s = get();
        const fullUrl = `${s.protocol}://${s.url}:${s.port}${s.path.startsWith('/') ? s.path : `/${s.path}`}`;
        errorNotified = false;
        set({ connecting: true, lastError: null });

        const options: IClientOptions = {
            clientId: s.clientId,
            username: s.username || undefined,
            password: s.password || undefined,
        };

        const c = mqtt.connect(fullUrl, options);
        client = c;

        c.on('connect', () => {
            errorNotified = false;
            set({ connected: true, connecting: false, lastError: null });
            // Re-subscribe any topics that were registered before this (re)connect.
            for (const sub of get().subTopics) {
                c.subscribe(sub.topic, { qos: sub.qos });
            }
        });
        c.on('close', () => set({ connected: false, connecting: false }));
        c.on('error', (err) => {
            const message = describeError(err);
            const handshakeFailed = !get().connected;

            /**
             * The broker rejected us (bad credentials, not authorized) or is unreachable.
             * mqtt.js would retry every second and re-emit this error each time, so stop here.
             */
            if (handshakeFailed) {
                c.end(true);
                if (client === c) client = null;
            }

            set({ connected: false, connecting: false, lastError: message });

            if (!errorNotified) {
                errorNotified = true;
                notify('error', message, i18n.t('websocket.title'));
            }
        });
        c.on('message', (topic, message, packet) => {
            set((state) => ({
                subLogs: [
                    { time: now(), topic, payload: message.toString(), qos: packet.qos as QoS },
                    ...state.subLogs,
                ],
            }));
        });
    },

    disconnect: () => {
        if (client) {
            client.end(true);
            client = null;
        }
        errorNotified = false;
        set({ connected: false, connecting: false, lastError: null });
    },

    publish: () => {
        const s = get();
        if (!client || !s.connected) return;
        client.publish(s.pubTopic, s.pubMsg, { qos: s.pubQoS });
        set((state) => ({
            pubLogs: [{ time: now(), topic: s.pubTopic, payload: s.pubMsg, qos: s.pubQoS }, ...state.pubLogs],
            pubMsg: '',
        }));
    },

    addSub: () => {
        const s = get();
        const topic = s.newSubTopic.trim();
        if (!client || !topic) return;
        client.subscribe(topic, { qos: s.newSubQoS }, (err) => {
            if (!err) {
                set((state) => ({
                    subLogs: [
                        { time: now(), topic, payload: i18n.t('websocket.subscribed'), qos: s.newSubQoS },
                        ...state.subLogs,
                    ],
                }));
            }
        });
        set((state) => ({
            subTopics: [...state.subTopics, { topic, qos: s.newSubQoS }],
            newSubTopic: '',
            newSubQoS: 0,
        }));
    },

    removeSub: (topic) => {
        if (client) client.unsubscribe(topic);
        set((state) => ({
            subTopics: state.subTopics.filter((x) => x.topic !== topic),
            subLogs: [{ time: now(), topic, payload: i18n.t('websocket.unsubscribed'), qos: 0 }, ...state.subLogs],
        }));
    },

    clearPubLogs: () => set({ pubLogs: [] }),
    clearSubLogs: () => set({ subLogs: [] }),
}));
