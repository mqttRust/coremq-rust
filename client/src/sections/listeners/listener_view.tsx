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
import Select from '@cloudscape-design/components/select';
import Textarea from '@cloudscape-design/components/textarea';
import StatusIndicator from '@cloudscape-design/components/status-indicator';
import TextFilter from '@cloudscape-design/components/text-filter';

import type { Listener, CreateListenerInput, ListenerProtocol } from 'src/types/listeners';
import { LISTENER_PROTOCOLS } from 'src/types/listeners';
import { useListenerStore, selectListeners } from 'src/stores/listener-store';
import { notify } from 'src/stores/notification-store';

const PROTO_COLOR: Record<string, 'blue' | 'green' | 'grey' | 'red'> = {
    tcp: 'blue',
    tls: 'green',
    ws: 'blue',
    wss: 'green',
};

type ProtoOption = { label: string; value: string };

const needsTls = (proto: string) => proto === 'tls' || proto === 'wss';

export function ListenerView() {
    const { t } = useTranslation();
    const listeners = useListenerStore(selectListeners);
    const loading = useListenerStore((s) => s.loading);
    const error = useListenerStore((s) => s.error);
    const fetch = useListenerStore((s) => s.fetch);
    const stop = useListenerStore((s) => s.stop);
    const create = useListenerStore((s) => s.create);
    const update = useListenerStore((s) => s.update);

    const [filterText, setFilterText] = useState('');
    const [confirm, setConfirm] = useState<Listener | null>(null);
    const [stopping, setStopping] = useState(false);

    // Create/Edit modal state
    const [modalOpen, setModalOpen] = useState(false);
    const [editingPort, setEditingPort] = useState<number | null>(null);
    const [creating, setCreating] = useState(false);
    const [name, setName] = useState('');
    const [protocol, setProtocol] = useState<ProtoOption>({ ...LISTENER_PROTOCOLS[0] });
    const [host, setHost] = useState('');
    const [port, setPort] = useState('');
    const [maxConn, setMaxConn] = useState('');
    const [cert, setCert] = useState('');
    const [key, setKey] = useState('');
    const [ca, setCa] = useState('');
    const [nameError, setNameError] = useState('');
    const [portError, setPortError] = useState('');
    const [certError, setCertError] = useState('');
    const [keyError, setKeyError] = useState('');

    useEffect(() => {
        fetch();
    }, [fetch]);

    useEffect(() => {
        if (error) notify('error', error, t('listeners.title'));
    }, [error, t]);

    const items = useMemo(() => {
        const q = filterText.trim().toLowerCase();
        if (!q) return listeners;
        return listeners.filter(
            (l) =>
                l.name.toLowerCase().includes(q) ||
                l.protocol.toLowerCase().includes(q) ||
                String(l.port).includes(q),
        );
    }, [listeners, filterText]);

    const totalConnections = useMemo(
        () => listeners.reduce((sum, l) => sum + (l.connections ?? 0), 0),
        [listeners],
    );

    const onStop = async () => {
        if (!confirm) return;
        setStopping(true);
        await stop(confirm.port);
        setStopping(false);
        setConfirm(null);
        notify('success', `Listener on port ${confirm.port} stopped.`);
    };

    const resetForm = () => {
        setName('');
        setProtocol({ ...LISTENER_PROTOCOLS[0] });
        setHost('');
        setPort('');
        setMaxConn('');
        setCert('');
        setKey('');
        setCa('');
        setNameError('');
        setPortError('');
        setCertError('');
        setKeyError('');
    };

    const openModal = () => {
        resetForm();
        setEditingPort(null);
        setModalOpen(true);
    };

    const openEdit = (l: Listener) => {
        resetForm();
        setEditingPort(l.port);
        setName(l.name);
        setProtocol(
            (LISTENER_PROTOCOLS.find((p) => p.value === l.protocol.toLowerCase()) as ProtoOption) ?? {
                ...LISTENER_PROTOCOLS[0],
            },
        );
        setHost(l.host === '0.0.0.0' ? '' : l.host);
        setPort(String(l.port));
        setMaxConn(l.max_connections ? String(l.max_connections) : '');
        setCert(l.tls?.cert ?? '');
        setKey(l.tls?.key ?? '');
        setCa(l.tls?.ca ?? '');
        setModalOpen(true);
    };

    const closeModal = () => {
        setModalOpen(false);
        setEditingPort(null);
        resetForm();
    };

    const handleCreate = async () => {
        const trimmedName = name.trim();
        const proto = protocol.value as ListenerProtocol;
        const tls = needsTls(proto);

        let invalid = false;

        if (!trimmedName) {
            setNameError('Name is required.');
            invalid = true;
        } else {
            setNameError('');
        }

        const portNum = Number(port);
        if (!port.trim() || !Number.isInteger(portNum) || portNum < 1 || portNum > 65535) {
            setPortError('Port must be a number between 1 and 65535.');
            invalid = true;
        } else {
            setPortError('');
        }

        if (tls) {
            if (!cert.trim()) {
                setCertError('Certificate is required for TLS/WSS.');
                invalid = true;
            } else {
                setCertError('');
            }
            if (!key.trim()) {
                setKeyError('Private key is required for TLS/WSS.');
                invalid = true;
            } else {
                setKeyError('');
            }
        } else {
            setCertError('');
            setKeyError('');
        }

        if (invalid) return;

        const input: CreateListenerInput = {
            name: trimmedName,
            protocol: proto,
            port: portNum,
        };
        if (host.trim()) input.host = host.trim();
        const mc = Number(maxConn);
        if (maxConn.trim() && Number.isInteger(mc) && mc > 0) input.max_connections = mc;
        if (tls) {
            input.tls = {
                cert: cert.trim(),
                key: key.trim(),
                ...(ca.trim() ? { ca: ca.trim() } : {}),
            };
        }

        setCreating(true);
        const ok = editingPort !== null ? await update(editingPort, input) : await create(input);
        setCreating(false);

        if (ok) {
            notify(
                'success',
                editingPort !== null
                    ? `Listener ${trimmedName} updated`
                    : `Listener ${trimmedName} started on port ${portNum}`,
            );
            closeModal();
        } else {
            notify('error', useListenerStore.getState().error ?? 'Failed to save listener');
        }
    };

    const showTlsFields = needsTls(protocol.value);

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description={`MQTT transport listeners configured on this broker. Stopping a listener disconnects all clients on its port. ${totalConnections} client(s) connected across all listeners.`}
                    counter={`(${listeners.length})`}
                    actions={
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="primary" onClick={openModal}>
                                Create listener
                            </Button>
                            <Button onClick={() => fetch()} loading={loading}>
                                Refresh
                            </Button>
                        </SpaceBetween>
                    }
                >
                    {t('listeners.title')}
                </Header>
            }
        >
            <Table<Listener>
                variant="container"
                loading={loading}
                loadingText="Loading listeners"
                items={items}
                trackBy="port"
                filter={
                    <TextFilter
                        filteringText={filterText}
                        filteringPlaceholder="Find listeners"
                        onChange={(e) => setFilterText(e.detail.filteringText)}
                    />
                }
                columnDefinitions={[
                    {
                        id: 'name',
                        header: t('listeners.name'),
                        cell: (l) => <Box fontWeight="bold">{l.name}</Box>,
                        sortingField: 'name',
                    },
                    {
                        id: 'protocol',
                        header: t('listeners.protocol'),
                        cell: (l) => (
                            <Badge color={PROTO_COLOR[l.protocol.toLowerCase()] ?? 'grey'}>
                                {l.protocol.toUpperCase()}
                            </Badge>
                        ),
                        sortingField: 'protocol',
                    },
                    { id: 'host', header: t('listeners.host'), cell: (l) => l.host },
                    {
                        id: 'port',
                        header: t('listeners.port'),
                        cell: (l) => <Box variant="samp">{l.port}</Box>,
                        sortingField: 'port',
                    },
                    {
                        id: 'tls',
                        header: 'TLS',
                        cell: (l) =>
                            l.tls ? (
                                <StatusIndicator type="success">Enabled</StatusIndicator>
                            ) : (
                                <StatusIndicator type="stopped">Disabled</StatusIndicator>
                            ),
                    },
                    {
                        id: 'connections',
                        header: 'Connections',
                        cell: (l) => (
                            <Badge color={l.connections > 0 ? 'blue' : 'grey'}>
                                {l.max_connections ? `${l.connections} / ${l.max_connections}` : l.connections}
                            </Badge>
                        ),
                        sortingField: 'connections',
                    },
                    {
                        id: 'limit',
                        header: 'Limit',
                        cell: (l) => (l.max_connections ? String(l.max_connections) : '∞'),
                    },
                    {
                        id: 'actions',
                        header: 'Actions',
                        cell: (l) => (
                            <SpaceBetween direction="horizontal" size="xs">
                                <Button variant="inline-link" onClick={() => openEdit(l)}>
                                    Edit
                                </Button>
                                <Button variant="inline-link" onClick={() => setConfirm(l)}>
                                    Stop
                                </Button>
                            </SpaceBetween>
                        ),
                    },
                ]}
                empty={
                    <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                        <SpaceBetween size="xs">
                            <b>No listeners</b>
                            <span>No MQTT listeners are currently running.</span>
                        </SpaceBetween>
                    </Box>
                }
            />

            <Modal
                visible={confirm !== null}
                onDismiss={() => setConfirm(null)}
                header="Stop listener"
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setConfirm(null)}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={stopping} onClick={onStop}>
                                Stop
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                Stop listener <b>{confirm?.name}</b> on port <b>{confirm?.port}</b>? All clients connected on
                this port will be disconnected.
            </Modal>

            <Modal
                visible={modalOpen}
                onDismiss={closeModal}
                header={editingPort !== null ? 'Edit listener' : 'Create listener'}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={closeModal}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={creating} onClick={handleCreate}>
                                {editingPort !== null ? 'Save changes' : 'Create'}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <SpaceBetween size="l">
                        <FormField label={t('listeners.name')} errorText={nameError}>
                            <Input
                                value={name}
                                onChange={(e) => setName(e.detail.value)}
                                placeholder="my-listener"
                            />
                        </FormField>
                        <FormField label={t('listeners.protocol')}>
                            <Select
                                selectedOption={protocol}
                                options={LISTENER_PROTOCOLS as unknown as ProtoOption[]}
                                onChange={(e) => setProtocol(e.detail.selectedOption as ProtoOption)}
                            />
                        </FormField>
                        <FormField label={t('listeners.host')} description="Optional. Defaults to 0.0.0.0.">
                            <Input
                                value={host}
                                onChange={(e) => setHost(e.detail.value)}
                                placeholder="0.0.0.0"
                            />
                        </FormField>
                        <FormField label={t('listeners.port')} errorText={portError}>
                            <Input
                                type="number"
                                value={port}
                                onChange={(e) => setPort(e.detail.value)}
                                placeholder="1883"
                            />
                        </FormField>
                        <FormField
                            label="Max connections"
                            description="Optional. Leave blank for unlimited."
                        >
                            <Input
                                type="number"
                                value={maxConn}
                                onChange={(e) => setMaxConn(e.detail.value)}
                                placeholder="unlimited"
                            />
                        </FormField>
                        {showTlsFields && (
                            <>
                                <FormField
                                    label="Certificate"
                                    description="PEM text or a server file path."
                                    errorText={certError}
                                >
                                    <Textarea
                                        rows={4}
                                        value={cert}
                                        onChange={(e) => setCert(e.detail.value)}
                                        placeholder="Paste PEM (-----BEGIN CERTIFICATE-----) or a server file path"
                                    />
                                </FormField>
                                <FormField
                                    label="Private key"
                                    description="PEM text or a server file path."
                                    errorText={keyError}
                                >
                                    <Textarea
                                        rows={4}
                                        value={key}
                                        onChange={(e) => setKey(e.detail.value)}
                                        placeholder="Paste PEM (-----BEGIN PRIVATE KEY-----) or a server file path"
                                    />
                                </FormField>
                                <FormField label="CA chain" description="Optional.">
                                    <Textarea
                                        rows={4}
                                        value={ca}
                                        onChange={(e) => setCa(e.detail.value)}
                                        placeholder="Paste PEM CA chain or a server file path (optional)"
                                    />
                                </FormField>
                            </>
                        )}
                    </SpaceBetween>
                </Form>
            </Modal>
        </ContentLayout>
    );
}

export default ListenerView;
