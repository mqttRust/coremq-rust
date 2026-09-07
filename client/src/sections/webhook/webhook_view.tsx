import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import ContentLayout from '@cloudscape-design/components/content-layout';
import Header from '@cloudscape-design/components/header';
import Table from '@cloudscape-design/components/table';
import Box from '@cloudscape-design/components/box';
import Button from '@cloudscape-design/components/button';
import Badge from '@cloudscape-design/components/badge';
import SpaceBetween from '@cloudscape-design/components/space-between';
import Modal from '@cloudscape-design/components/modal';
import Form from '@cloudscape-design/components/form';
import FormField from '@cloudscape-design/components/form-field';
import Input from '@cloudscape-design/components/input';
import Multiselect, { type MultiselectProps } from '@cloudscape-design/components/multiselect';
import Toggle from '@cloudscape-design/components/toggle';
import AttributeEditor from '@cloudscape-design/components/attribute-editor';
import StatusIndicator from '@cloudscape-design/components/status-indicator';

import type { Webhook, WebhookInput, HttpHeader } from 'src/types/webhooks';
import { WEBHOOK_EVENTS, TOPIC_FILTER_EVENT } from 'src/types/webhooks';
import { useWebhookStore, selectWebhooks } from 'src/stores/webhook-store';
import { testWebhook } from 'src/services/webhooks';
import { notify } from 'src/stores/notification-store';

const EVENT_OPTIONS: MultiselectProps.Option[] = WEBHOOK_EVENTS.map((e) => ({
    label: e.label,
    value: e.value,
}));

/** value -> short label, for compact badges in the table. */
const SHORT_LABEL: Record<string, string> = Object.fromEntries(
    WEBHOOK_EVENTS.map((e) => [e.value, e.short]),
);

export function WebhookView() {
    const { t } = useTranslation();
    const webhooks = useWebhookStore(selectWebhooks);
    const loading = useWebhookStore((s) => s.loading);
    const error = useWebhookStore((s) => s.error);
    const fetch = useWebhookStore((s) => s.fetch);
    const create = useWebhookStore((s) => s.create);
    const update = useWebhookStore((s) => s.update);
    const remove = useWebhookStore((s) => s.remove);

    // Form / modal state
    const [modalOpen, setModalOpen] = useState(false);
    const [editing, setEditing] = useState<Webhook | null>(null);
    const [submitting, setSubmitting] = useState(false);

    const [name, setName] = useState('');
    const [url, setUrl] = useState('');
    const [selectedEvents, setSelectedEvents] = useState<MultiselectProps.Option[]>([]);
    const [topicFilter, setTopicFilter] = useState('');
    const [enabled, setEnabled] = useState(true);
    const [secret, setSecret] = useState('');
    const [headers, setHeaders] = useState<HttpHeader[]>([]);

    const [nameError, setNameError] = useState('');
    const [urlError, setUrlError] = useState('');
    const [eventsError, setEventsError] = useState('');

    // Delete + test state
    const [confirm, setConfirm] = useState<Webhook | null>(null);
    const [deleting, setDeleting] = useState(false);
    const [testingId, setTestingId] = useState<string | null>(null);

    useEffect(() => {
        fetch();
    }, [fetch]);

    useEffect(() => {
        if (error) notify('error', error, t('webhook.title'));
    }, [error, t]);

    const showTopicFilter = useMemo(
        () => selectedEvents.some((o) => o.value === TOPIC_FILTER_EVENT),
        [selectedEvents],
    );

    const resetForm = () => {
        setName('');
        setUrl('');
        setSelectedEvents([]);
        setTopicFilter('');
        setEnabled(true);
        setSecret('');
        setHeaders([]);
        setNameError('');
        setUrlError('');
        setEventsError('');
    };

    const updateHeader = (index: number, field: 'key' | 'value', value: string) =>
        setHeaders((prev) => prev.map((h, i) => (i === index ? { ...h, [field]: value } : h)));

    const openCreate = () => {
        setEditing(null);
        resetForm();
        setModalOpen(true);
    };

    const openEdit = (w: Webhook) => {
        setEditing(w);
        setName(w.name);
        setUrl(w.url);
        setSelectedEvents(
            EVENT_OPTIONS.filter((o) => w.events.includes(o.value as string)),
        );
        setTopicFilter(w.topic_filter ?? '');
        setEnabled(w.enabled);
        setSecret(w.secret ?? '');
        setHeaders(w.headers ?? []);
        setNameError('');
        setUrlError('');
        setEventsError('');
        setModalOpen(true);
    };

    const closeModal = () => {
        setModalOpen(false);
        setEditing(null);
        resetForm();
    };

    const handleSubmit = async () => {
        const trimmedName = name.trim();
        const trimmedUrl = url.trim();
        const events = selectedEvents.map((o) => o.value as string);

        let invalid = false;
        if (!trimmedName) {
            setNameError('Name is required.');
            invalid = true;
        } else {
            setNameError('');
        }
        if (!/^https?:\/\//i.test(trimmedUrl)) {
            setUrlError('URL must start with http:// or https://');
            invalid = true;
        } else {
            setUrlError('');
        }
        if (events.length === 0) {
            setEventsError('Select at least one event.');
            invalid = true;
        } else {
            setEventsError('');
        }
        if (invalid) return;

        const filter = topicFilter.trim();
        const input: WebhookInput = {
            name: trimmedName,
            url: trimmedUrl,
            events,
            topic_filter:
                events.includes(TOPIC_FILTER_EVENT) && filter ? filter : null,
            headers: headers.filter((h) => h.key.trim()).map((h) => ({ key: h.key.trim(), value: h.value })),
            enabled,
            secret: secret.trim() ? secret.trim() : null,
        };

        setSubmitting(true);
        const ok = editing
            ? await update(editing.id, input)
            : await create(input);
        setSubmitting(false);

        if (ok) {
            notify(
                'success',
                editing
                    ? `Webhook ${trimmedName} updated`
                    : `Webhook ${trimmedName} created`,
            );
            closeModal();
        } else {
            notify('error', useWebhookStore.getState().error ?? 'Failed to save webhook');
        }
    };

    const handleTest = async (w: Webhook) => {
        setTestingId(w.id);
        try {
            const res = await testWebhook(w.id);
            notify('success', `Test delivered: ${res.data ?? 'OK'}`);
        } catch (err: any) {
            notify(
                'error',
                'Test failed: ' + (err?.response?.data?.message || err?.message || 'Unknown error'),
            );
        } finally {
            setTestingId(null);
        }
    };

    const onDelete = async () => {
        if (!confirm) return;
        setDeleting(true);
        const ok = await remove(confirm.id);
        setDeleting(false);
        if (ok) {
            notify('success', `Webhook ${confirm.name} deleted`);
            setConfirm(null);
        } else {
            notify('error', useWebhookStore.getState().error ?? 'Failed to delete webhook');
        }
    };

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="Webhooks fire an HTTP POST to your endpoints whenever a broker event occurs, so external systems can react in real time."
                    counter={`(${webhooks.length})`}
                    actions={
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="primary" onClick={openCreate}>
                                New webhook
                            </Button>
                            <Button onClick={() => fetch()} loading={loading}>
                                Refresh
                            </Button>
                        </SpaceBetween>
                    }
                >
                    {t('webhook.title')}
                </Header>
            }
        >
            <Table<Webhook>
                variant="container"
                loading={loading}
                loadingText="Loading webhooks"
                items={webhooks}
                trackBy="id"
                columnDefinitions={[
                    {
                        id: 'name',
                        header: 'Name',
                        cell: (w) => <Box fontWeight="bold">{w.name}</Box>,
                        sortingField: 'name',
                    },
                    {
                        id: 'url',
                        header: 'URL',
                        cell: (w) => (
                            <Box variant="samp">
                                <span title={w.url}>{w.url}</span>
                            </Box>
                        ),
                    },
                    {
                        id: 'events',
                        header: 'Events',
                        cell: (w) => (
                            <SpaceBetween direction="horizontal" size="xxs">
                                {w.events.map((ev) => (
                                    <Badge key={ev} color="blue">
                                        {SHORT_LABEL[ev] ?? ev}
                                    </Badge>
                                ))}
                            </SpaceBetween>
                        ),
                    },
                    {
                        id: 'status',
                        header: 'Status',
                        cell: (w) =>
                            w.enabled ? (
                                <StatusIndicator type="success">Enabled</StatusIndicator>
                            ) : (
                                <StatusIndicator type="stopped">Disabled</StatusIndicator>
                            ),
                    },
                    {
                        id: 'created',
                        header: 'Created',
                        cell: (w) => w.created_at,
                    },
                    {
                        id: 'actions',
                        header: 'Actions',
                        cell: (w) => (
                            <SpaceBetween direction="horizontal" size="xs">
                                <Button
                                    variant="inline-link"
                                    loading={testingId === w.id}
                                    onClick={() => handleTest(w)}
                                >
                                    Test
                                </Button>
                                <Button variant="inline-link" onClick={() => openEdit(w)}>
                                    Edit
                                </Button>
                                <Button variant="inline-link" onClick={() => setConfirm(w)}>
                                    Delete
                                </Button>
                            </SpaceBetween>
                        ),
                    },
                ]}
                empty={
                    <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                        <SpaceBetween size="xs">
                            <b>No webhooks</b>
                            <span>No webhooks have been registered yet.</span>
                        </SpaceBetween>
                    </Box>
                }
            />

            <Modal
                visible={modalOpen}
                onDismiss={closeModal}
                header={editing ? 'Edit webhook' : 'New webhook'}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={closeModal}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={submitting} onClick={handleSubmit}>
                                {editing ? 'Save changes' : 'Create webhook'}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <SpaceBetween size="l">
                        <FormField label="Name" errorText={nameError}>
                            <Input
                                value={name}
                                onChange={(e) => setName(e.detail.value)}
                                placeholder="My webhook"
                            />
                        </FormField>
                        <FormField label="URL" errorText={urlError}>
                            <Input
                                value={url}
                                onChange={(e) => setUrl(e.detail.value)}
                                placeholder="https://example.com/hook"
                            />
                        </FormField>
                        <FormField label="Events" errorText={eventsError}>
                            <Multiselect
                                selectedOptions={selectedEvents}
                                options={EVENT_OPTIONS}
                                onChange={(e) =>
                                    setSelectedEvents([...e.detail.selectedOptions])
                                }
                                placeholder="Choose events"
                                tokenLimit={5}
                            />
                        </FormField>
                        {showTopicFilter && (
                            <FormField
                                label="Topic filter"
                                description="MQTT filter, e.g. sensors/#"
                            >
                                <Input
                                    value={topicFilter}
                                    onChange={(e) => setTopicFilter(e.detail.value)}
                                    placeholder="sensors/#"
                                />
                            </FormField>
                        )}
                        <FormField label="Enabled">
                            <Toggle
                                checked={enabled}
                                onChange={(e) => setEnabled(e.detail.checked)}
                            >
                                {enabled ? 'Enabled' : 'Disabled'}
                            </Toggle>
                        </FormField>
                        <FormField
                            label="Secret"
                            description="Optional HMAC-SHA256 signing secret"
                        >
                            <Input
                                type="password"
                                value={secret}
                                onChange={(e) => setSecret(e.detail.value)}
                                placeholder="Optional"
                            />
                        </FormField>
                        <FormField
                            label="Custom HTTP headers"
                            description="Sent with every delivery to this webhook (e.g. Authorization)."
                        >
                            <AttributeEditor
                                items={headers}
                                addButtonText="Add header"
                                removeButtonText="Remove"
                                empty="No custom headers"
                                definition={[
                                    {
                                        label: 'Name',
                                        control: (item: HttpHeader, i: number) => (
                                            <Input
                                                value={item.key}
                                                placeholder="Header-Name"
                                                onChange={(e) => updateHeader(i, 'key', e.detail.value)}
                                            />
                                        ),
                                    },
                                    {
                                        label: 'Value',
                                        control: (item: HttpHeader, i: number) => (
                                            <Input
                                                value={item.value}
                                                placeholder="value"
                                                onChange={(e) => updateHeader(i, 'value', e.detail.value)}
                                            />
                                        ),
                                    },
                                ]}
                                onAddButtonClick={() => setHeaders((prev) => [...prev, { key: '', value: '' }])}
                                onRemoveButtonClick={({ detail }) =>
                                    setHeaders((prev) => prev.filter((_, i) => i !== detail.itemIndex))
                                }
                            />
                        </FormField>
                    </SpaceBetween>
                </Form>
            </Modal>

            <Modal
                visible={confirm !== null}
                onDismiss={() => setConfirm(null)}
                header="Delete webhook"
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setConfirm(null)}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={deleting} onClick={onDelete}>
                                Delete
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                Delete webhook <b>{confirm?.name}</b>? It will stop receiving broker events. This
                cannot be undone.
            </Modal>
        </ContentLayout>
    );
}

export default WebhookView;
