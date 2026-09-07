export type HttpHeader = {
    key: string;
    value: string;
};

export type Webhook = {
    id: string;
    name: string;
    url: string;
    events: string[];
    topic_filter?: string | null;
    headers: HttpHeader[];
    enabled: boolean;
    secret?: string | null;
    created_at: string;
};

export type WebhookInput = {
    name: string;
    url: string;
    events: string[];
    topic_filter?: string | null;
    headers: HttpHeader[];
    enabled: boolean;
    secret?: string | null;
};

export type WebhookEventOption = {
    value: string;
    label: string;
    /** Short label for compact display (badges). */
    short: string;
};

/** The five broker events a webhook can subscribe to. */
export const WEBHOOK_EVENTS: WebhookEventOption[] = [
    { value: 'client.connected', label: 'Client connected', short: 'Connected' },
    { value: 'client.disconnected', label: 'Client disconnected', short: 'Disconnected' },
    { value: 'message.published', label: 'Message published', short: 'Published' },
    { value: 'subscription.created', label: 'Subscription created', short: 'Sub created' },
    { value: 'subscription.removed', label: 'Subscription removed', short: 'Sub removed' },
];

/** Event whose delivery can be narrowed by `topic_filter`. */
export const TOPIC_FILTER_EVENT = 'message.published';
