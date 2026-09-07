import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import ContentLayout from '@cloudscape-design/components/content-layout';
import Header from '@cloudscape-design/components/header';
import Table from '@cloudscape-design/components/table';
import Box from '@cloudscape-design/components/box';
import Button from '@cloudscape-design/components/button';
import Badge from '@cloudscape-design/components/badge';
import SpaceBetween from '@cloudscape-design/components/space-between';
import Modal from '@cloudscape-design/components/modal';
import StatusIndicator from '@cloudscape-design/components/status-indicator';
import TextFilter from '@cloudscape-design/components/text-filter';
import Pagination from '@cloudscape-design/components/pagination';
import Select from '@cloudscape-design/components/select';
import KeyValuePairs from '@cloudscape-design/components/key-value-pairs';

import { useShallow } from 'zustand/react/shallow';

import type { Session } from 'src/types/sessions';
import { useSessionStore, selectSessions, selectSessionPagination } from 'src/stores/session-store';
import { notify } from 'src/stores/notification-store';

const PAGE_SIZE_OPTIONS = [
    { label: '10 / page', value: '10' },
    { label: '20 / page', value: '20' },
    { label: '50 / page', value: '50' },
];

const subCount = (s: Session) => Object.keys(s.subscriptions ?? {}).length;

export function SessionView() {
    const { t } = useTranslation();

    const sessions = useSessionStore(selectSessions);
    const { page, size, totalPages, totalElements } = useSessionStore(useShallow(selectSessionPagination));
    const loading = useSessionStore((s) => s.loading);
    const error = useSessionStore((s) => s.error);
    const fetch = useSessionStore((s) => s.fetch);
    const disconnect = useSessionStore((s) => s.disconnect);
    const setSize = useSessionStore((s) => s.setSize);

    const [filterText, setFilterText] = useState('');
    const [selected, setSelected] = useState<Session | null>(null);
    const [disconnecting, setDisconnecting] = useState(false);

    useEffect(() => {
        fetch(0, size);
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [fetch]);

    useEffect(() => {
        if (error) notify('error', error, t('sessions.title'));
    }, [error, t]);

    const onDisconnect = async () => {
        if (!selected) return;
        const clientId = selected.client_id;
        setDisconnecting(true);
        await disconnect(clientId);
        setDisconnecting(false);
        setSelected(null);
        notify('success', `Client "${clientId}" disconnected.`);
    };

    const subscriptionEntries = selected ? Object.entries(selected.subscriptions ?? {}) : [];

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="Connected MQTT clients on this broker. Select Details to inspect a session or disconnect a client."
                    counter={`(${totalElements})`}
                    actions={
                        <Button
                            onClick={() => fetch(page, size, filterText.trim() || undefined)}
                            loading={loading}
                        >
                            {t('sessions.refresh')}
                        </Button>
                    }
                >
                    {t('sessions.title')}
                </Header>
            }
        >
            <Table<Session>
                variant="container"
                loading={loading}
                loadingText="Loading sessions"
                items={sessions}
                trackBy="client_id"
                selectionType="single"
                selectedItems={selected ? [selected] : []}
                onSelectionChange={(e) => setSelected(e.detail.selectedItems[0] ?? null)}
                filter={
                    <TextFilter
                        filteringText={filterText}
                        filteringPlaceholder={t('sessions.search') + '...'}
                        onChange={(e) => setFilterText(e.detail.filteringText)}
                        onDelayedChange={(e) => fetch(0, size, e.detail.filteringText.trim() || undefined)}
                    />
                }
                pagination={
                    <Pagination
                        currentPageIndex={page + 1}
                        pagesCount={Math.max(totalPages, 1)}
                        onChange={(e) =>
                            fetch(e.detail.currentPageIndex - 1, size, filterText.trim() || undefined)
                        }
                    />
                }
                preferences={
                    <Select
                        selectedOption={
                            PAGE_SIZE_OPTIONS.find((o) => o.value === String(size)) ?? PAGE_SIZE_OPTIONS[0]
                        }
                        options={PAGE_SIZE_OPTIONS}
                        onChange={(e) => {
                            const newSize = Number(e.detail.selectedOption.value);
                            setSize(newSize);
                            fetch(0, newSize, filterText.trim() || undefined);
                        }}
                    />
                }
                columnDefinitions={[
                    {
                        id: 'index',
                        header: t('sessions.id'),
                        cell: (s) => (
                            <Box color="text-body-secondary">
                                {page * size + sessions.indexOf(s) + 1}
                            </Box>
                        ),
                    },
                    {
                        id: 'clientId',
                        header: t('sessions.clientId'),
                        cell: (s) => <Box variant="samp">{s.client_id}</Box>,
                    },
                    {
                        id: 'username',
                        header: t('sessions.username'),
                        cell: (s) =>
                            s.username || <Box color="text-body-secondary">--</Box>,
                    },
                    {
                        id: 'remoteAddr',
                        header: t('sessions.remoteAddress'),
                        cell: (s) => <Box variant="samp">{s.remote_addr}</Box>,
                    },
                    {
                        id: 'port',
                        header: t('sessions.port'),
                        cell: (s) => <Box variant="samp">{s.connected_port}</Box>,
                    },
                    {
                        id: 'connectedAt',
                        header: t('sessions.connectedAt'),
                        cell: (s) => <Box color="text-body-secondary">{s.connected_at}</Box>,
                    },
                    {
                        id: 'subscriptions',
                        header: t('sessions.subscriptions'),
                        cell: (s) => (
                            <Badge color={subCount(s) > 0 ? 'blue' : 'grey'}>{subCount(s)}</Badge>
                        ),
                    },
                    {
                        id: 'actions',
                        header: t('sessions.actions'),
                        cell: (s) => (
                            <Button
                                variant="inline-link"
                                onClick={() => setSelected(s)}
                            >
                                Details
                            </Button>
                        ),
                    },
                ]}
                empty={
                    <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                        <SpaceBetween size="xs">
                            <b>{t('sessions.empty')}</b>
                            <span>No MQTT clients are currently connected.</span>
                        </SpaceBetween>
                    </Box>
                }
            />

            <Modal
                visible={selected !== null}
                onDismiss={() => setSelected(null)}
                header="Session details"
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setSelected(null)}>
                                Close
                            </Button>
                            <Button variant="primary" loading={disconnecting} onClick={onDisconnect}>
                                Disconnect
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                {selected && (
                    <SpaceBetween size="l">
                        <KeyValuePairs
                            columns={2}
                            items={[
                                {
                                    label: t('sessions.clientId'),
                                    value: <Box variant="samp">{selected.client_id}</Box>,
                                },
                                {
                                    label: t('sessions.username'),
                                    value: selected.username || '--',
                                },
                                {
                                    label: 'Clean session',
                                    value: selected.clean_session ? (
                                        <StatusIndicator type="success">Yes</StatusIndicator>
                                    ) : (
                                        <StatusIndicator type="stopped">No</StatusIndicator>
                                    ),
                                },
                                {
                                    label: t('sessions.remoteAddress'),
                                    value: <Box variant="samp">{selected.remote_addr}</Box>,
                                },
                                {
                                    label: t('sessions.port'),
                                    value: <Box variant="samp">{selected.connected_port}</Box>,
                                },
                                {
                                    label: t('sessions.connectedAt'),
                                    value: selected.connected_at,
                                },
                            ]}
                        />

                        <div>
                            <Box variant="h4" padding={{ bottom: 'xs' }}>
                                {t('sessions.subscriptions')} ({subscriptionEntries.length})
                            </Box>
                            {subscriptionEntries.length === 0 ? (
                                <Box color="text-body-secondary">No active subscriptions.</Box>
                            ) : (
                                <Table<[string, any]>
                                    variant="embedded"
                                    items={subscriptionEntries}
                                    trackBy={(entry) => entry[0]}
                                    columnDefinitions={[
                                        {
                                            id: 'topic',
                                            header: t('topics.topic'),
                                            cell: (entry) => <Box variant="samp">{entry[0]}</Box>,
                                        },
                                        {
                                            id: 'qos',
                                            header: 'QoS',
                                            cell: (entry) =>
                                                entry[1]?.qos !== undefined ? (
                                                    <Badge>{`QoS ${entry[1].qos}`}</Badge>
                                                ) : (
                                                    '--'
                                                ),
                                        },
                                    ]}
                                />
                            )}
                        </div>
                    </SpaceBetween>
                )}
            </Modal>
        </ContentLayout>
    );
}

export default SessionView;
