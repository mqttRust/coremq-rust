import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import Alert from '@cloudscape-design/components/alert';
import Box from '@cloudscape-design/components/box';
import Button from '@cloudscape-design/components/button';
import ColumnLayout from '@cloudscape-design/components/column-layout';
import Container from '@cloudscape-design/components/container';
import ContentLayout from '@cloudscape-design/components/content-layout';
import Form from '@cloudscape-design/components/form';
import FormField from '@cloudscape-design/components/form-field';
import Header from '@cloudscape-design/components/header';
import Input from '@cloudscape-design/components/input';
import Modal from '@cloudscape-design/components/modal';
import SpaceBetween from '@cloudscape-design/components/space-between';
import StatusIndicator, {
    type StatusIndicatorProps,
} from '@cloudscape-design/components/status-indicator';
import Table from '@cloudscape-design/components/table';
import Tabs from '@cloudscape-design/components/tabs';

import { useClusterStore } from 'src/stores/cluster-store';
import type { ClusterNode, FederationLink, NodeState } from 'src/types/cluster';

/** Poll interval for the cluster view; membership changes within seconds. */
const REFRESH_MS = 5000;

const NODE_STATUS: Record<NodeState, StatusIndicatorProps.Type> = {
    alive: 'success',
    suspect: 'warning',
    dead: 'error',
    left: 'stopped',
};

const LINK_STATUS: Record<FederationLink['state'], StatusIndicatorProps.Type> = {
    up: 'success',
    connecting: 'loading',
    down: 'error',
    standby: 'stopped',
};

/** Split a comma or newline separated list into trimmed, non-empty entries. */
const parseList = (raw: string): string[] =>
    raw
        .split(/[\n,]/)
        .map((s) => s.trim())
        .filter(Boolean);

export default function ClusterView() {
    const { t } = useTranslation();

    const status = useClusterStore((s) => s.status);
    const nodes = useClusterStore((s) => s.nodes);
    const routes = useClusterStore((s) => s.routes);
    const links = useClusterStore((s) => s.links);
    const loading = useClusterStore((s) => s.loading);
    const loaded = useClusterStore((s) => s.loaded);
    const fetch = useClusterStore((s) => s.fetch);
    const join = useClusterStore((s) => s.join);
    const evict = useClusterStore((s) => s.evict);
    const addLink = useClusterStore((s) => s.addLink);
    const removeLink = useClusterStore((s) => s.removeLink);

    const [joinOpen, setJoinOpen] = useState(false);
    const [joinAddress, setJoinAddress] = useState('');
    const [joinError, setJoinError] = useState('');
    const [joining, setJoining] = useState(false);

    const [evictTarget, setEvictTarget] = useState<ClusterNode | null>(null);
    const [evicting, setEvicting] = useState(false);

    const [linkOpen, setLinkOpen] = useState(false);
    const [linkName, setLinkName] = useState('');
    const [linkEndpoints, setLinkEndpoints] = useState('');
    const [linkForward, setLinkForward] = useState('');
    const [linkAccept, setLinkAccept] = useState('');
    const [linkSecret, setLinkSecret] = useState('');
    const [linkError, setLinkError] = useState('');
    const [savingLink, setSavingLink] = useState(false);

    const [removeTarget, setRemoveTarget] = useState<FederationLink | null>(null);
    const [removingLink, setRemovingLink] = useState(false);

    useEffect(() => {
        fetch();
        const timer = setInterval(fetch, REFRESH_MS);
        return () => clearInterval(timer);
    }, [fetch]);

    const onJoin = async () => {
        const address = joinAddress.trim();
        if (!address) {
            setJoinError(t('cluster.address_required'));
            return;
        }
        /* host:port — the peer port, not the MQTT or admin one. */
        if (!/^[^\s:]+:\d+$/.test(address)) {
            setJoinError(t('cluster.address_invalid'));
            return;
        }

        setJoinError('');
        setJoining(true);
        const ok = await join(address);
        setJoining(false);
        if (ok) {
            setJoinAddress('');
            setJoinOpen(false);
        }
    };

    const onConfirmEvict = async () => {
        if (!evictTarget) return;
        setEvicting(true);
        await evict(evictTarget.id);
        setEvicting(false);
        setEvictTarget(null);
    };

    const openLinkModal = () => {
        setLinkName('');
        setLinkEndpoints('');
        setLinkForward('');
        setLinkAccept('');
        setLinkSecret('');
        setLinkError('');
        setLinkOpen(true);
    };

    const onCreateLink = async () => {
        const name = linkName.trim();
        const endpoints = parseList(linkEndpoints);

        if (!name) {
            setLinkError(t('cluster.link_name_required'));
            return;
        }
        if (endpoints.length === 0) {
            setLinkError(t('cluster.link_endpoints_required'));
            return;
        }

        setLinkError('');
        setSavingLink(true);
        const ok = await addLink({
            name,
            endpoints,
            forward: parseList(linkForward),
            accept: parseList(linkAccept),
            secret: linkSecret,
        });
        setSavingLink(false);
        if (ok) setLinkOpen(false);
    };

    const onConfirmRemoveLink = async () => {
        if (!removeTarget) return;
        setRemovingLink(true);
        await removeLink(removeTarget.name);
        setRemovingLink(false);
        setRemoveTarget(null);
    };

    /*
      Until the first response lands we cannot tell "disabled" from "not loaded
      yet", and flashing a disabled banner on every visit reads as a fault.
    */
    if (!loaded) {
        return (
            <ContentLayout header={<Header variant="h1">{t('cluster.title')}</Header>}>
                <Container>
                    <Box textAlign="center" padding={{ vertical: 'xxl' }}>
                        <StatusIndicator type="loading">{t('cluster.loading')}</StatusIndicator>
                    </Box>
                </Container>
            </ContentLayout>
        );
    }

    if (!status.enabled) {
        return (
            <ContentLayout
                header={
                    <Header variant="h1" description={t('cluster.description')}>
                        {t('cluster.title')}
                    </Header>
                }
            >
                <Alert type="info" header={t('cluster.disabled_title')}>
                    <SpaceBetween size="s">
                        <span>{t('cluster.disabled_body')}</span>
                        <Box variant="code">
                            {[
                                'cluster:',
                                '  enabled: true',
                                '  name: prod',
                                '  bind: "0.0.0.0:4370"',
                                '  discovery:',
                                '    type: static',
                                '    seeds: ["10.0.0.1:4370", "10.0.0.2:4370"]',
                            ].join('\n')}
                        </Box>
                    </SpaceBetween>
                </Alert>
            </ContentLayout>
        );
    }

    const nodesTab = (
        <Table<ClusterNode>
            variant="container"
            loading={loading && nodes.length === 0}
            loadingText={t('cluster.loading')}
            items={nodes}
            trackBy="id"
            header={
                <Header
                    variant="h2"
                    counter={`(${nodes.length})`}
                    description={t('cluster.nodes_description')}
                    actions={
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button onClick={() => fetch()} loading={loading}>
                                {t('cluster.refresh')}
                            </Button>
                            <Button variant="primary" onClick={() => setJoinOpen(true)}>
                                {t('cluster.join_node')}
                            </Button>
                        </SpaceBetween>
                    }
                >
                    {t('cluster.nodes')}
                </Header>
            }
            columnDefinitions={[
                {
                    id: 'id',
                    header: t('cluster.node_id'),
                    cell: (n) => (
                        <SpaceBetween direction="horizontal" size="xs">
                            <Box fontWeight="bold">{n.id}</Box>
                            {n.is_self && <Box color="text-body-secondary">({t('cluster.this_node')})</Box>}
                        </SpaceBetween>
                    ),
                },
                {
                    id: 'state',
                    header: t('cluster.state'),
                    cell: (n) => (
                        <StatusIndicator type={NODE_STATUS[n.state] ?? 'stopped'}>
                            {t(`cluster.state_${n.state}`)}
                        </StatusIndicator>
                    ),
                },
                { id: 'addr', header: t('cluster.address'), cell: (n) => n.advertise_addr },
                { id: 'routes', header: t('cluster.routes'), cell: (n) => n.routes },
                {
                    id: 'last_seen',
                    header: t('cluster.last_seen'),
                    cell: (n) => (n.is_self ? '—' : `${n.last_seen_secs}s`),
                },
                { id: 'incarnation', header: t('cluster.incarnation'), cell: (n) => n.incarnation },
                {
                    id: 'dropped',
                    header: t('cluster.dropped'),
                    cell: (n) => n.dropped_messages,
                },
                {
                    id: 'actions',
                    header: t('cluster.actions'),
                    cell: (n) =>
                        n.is_self ? null : (
                            <Button variant="inline-link" onClick={() => setEvictTarget(n)}>
                                {t('cluster.evict')}
                            </Button>
                        ),
                },
            ]}
            empty={
                <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                    <SpaceBetween size="xs">
                        <b>{t('cluster.no_nodes')}</b>
                        <span>{t('cluster.no_nodes_hint')}</span>
                    </SpaceBetween>
                </Box>
            }
        />
    );

    const routesTab = (
        <Table
            variant="container"
            loading={loading && routes.length === 0}
            loadingText={t('cluster.loading')}
            items={routes}
            trackBy={(r) => `${r.node}:${r.filter}`}
            header={
                <Header
                    variant="h2"
                    counter={`(${routes.length})`}
                    description={t('cluster.routes_description')}
                >
                    {t('cluster.route_table')}
                </Header>
            }
            columnDefinitions={[
                {
                    id: 'filter',
                    header: t('cluster.filter'),
                    cell: (r) => <Box variant="samp">{r.filter}</Box>,
                },
                { id: 'node', header: t('cluster.node'), cell: (r) => r.node },
            ]}
            empty={
                <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                    <SpaceBetween size="xs">
                        <b>{t('cluster.no_routes')}</b>
                        <span>{t('cluster.no_routes_hint')}</span>
                    </SpaceBetween>
                </Box>
            }
        />
    );

    const federationTab = (
        <Table<FederationLink>
            variant="container"
            loading={loading && links.length === 0}
            loadingText={t('cluster.loading')}
            items={links}
            trackBy="name"
            header={
                <Header
                    variant="h2"
                    counter={`(${links.length})`}
                    description={t('cluster.federation_description')}
                    actions={
                        <Button variant="primary" onClick={openLinkModal}>
                            {t('cluster.add_link')}
                        </Button>
                    }
                >
                    {t('cluster.federation')}
                </Header>
            }
            columnDefinitions={[
                {
                    id: 'name',
                    header: t('cluster.link_name'),
                    cell: (l) => <Box fontWeight="bold">{l.name}</Box>,
                },
                {
                    id: 'state',
                    header: t('cluster.state'),
                    cell: (l) => (
                        <StatusIndicator type={LINK_STATUS[l.state] ?? 'stopped'}>
                            {t(`cluster.link_${l.state}`)}
                        </StatusIndicator>
                    ),
                },
                {
                    id: 'endpoints',
                    header: t('cluster.endpoints'),
                    cell: (l) => l.endpoints.join(', ') || '—',
                },
                {
                    id: 'forward',
                    header: t('cluster.forward'),
                    cell: (l) => <Box variant="samp">{l.forward.join(', ') || '—'}</Box>,
                },
                {
                    id: 'accept',
                    header: t('cluster.accept'),
                    cell: (l) => <Box variant="samp">{l.accept.join(', ') || '—'}</Box>,
                },
                {
                    id: 'counters',
                    header: t('cluster.messages'),
                    cell: (l) => `${l.sent} / ${l.received} / ${l.dropped}`,
                },
                {
                    id: 'actions',
                    header: t('cluster.actions'),
                    cell: (l) => (
                        <Button variant="inline-link" onClick={() => setRemoveTarget(l)}>
                            {t('cluster.remove')}
                        </Button>
                    ),
                },
            ]}
            empty={
                <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                    <SpaceBetween size="xs">
                        <b>{t('cluster.no_links')}</b>
                        <span>{t('cluster.no_links_hint')}</span>
                    </SpaceBetween>
                </Box>
            }
        />
    );

    const suspects = nodes.filter((n) => n.state === 'suspect').length;
    const dead = nodes.filter((n) => n.state === 'dead').length;

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description={t('cluster.description')}
                    counter={`(${status.members_alive}/${status.members_total})`}
                >
                    {t('cluster.title')}
                </Header>
            }
        >
            <SpaceBetween size="l">
                {suspects > 0 && (
                    <Alert type="warning" header={t('cluster.suspect_title')}>
                        {t('cluster.suspect_body', { count: suspects })}
                    </Alert>
                )}

                {dead > 0 && (
                    <Alert type="error" header={t('cluster.dead_title')}>
                        {t('cluster.dead_body', { count: dead })}
                    </Alert>
                )}

                <Container header={<Header variant="h2">{t('cluster.overview')}</Header>}>
                    <ColumnLayout columns={4} variant="text-grid">
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.cluster_name')}</Box>
                            <Box fontWeight="bold">{status.cluster}</Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.this_node')}</Box>
                            <Box variant="samp">{status.node_id}</Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.address')}</Box>
                            <Box variant="samp">{status.advertise_addr}</Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.discovery')}</Box>
                            <Box>{status.discovery}</Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.members')}</Box>
                            <Box>
                                {status.members_alive} / {status.members_total}
                            </Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.routes')}</Box>
                            <Box>{status.routes_total}</Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.incarnation')}</Box>
                            <Box>{status.incarnation}</Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">{t('cluster.federation_owner')}</Box>
                            <Box>
                                {status.is_federation_owner ? t('cluster.yes') : t('cluster.no')}
                            </Box>
                        </div>
                    </ColumnLayout>
                </Container>

                <Tabs
                    tabs={[
                        { id: 'nodes', label: t('cluster.nodes'), content: nodesTab },
                        { id: 'routes', label: t('cluster.route_table'), content: routesTab },
                        {
                            id: 'federation',
                            label: t('cluster.federation'),
                            content: federationTab,
                        },
                    ]}
                />
            </SpaceBetween>

            <Modal
                visible={joinOpen}
                onDismiss={() => setJoinOpen(false)}
                header={t('cluster.join_node')}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setJoinOpen(false)}>
                                {t('cluster.cancel')}
                            </Button>
                            <Button variant="primary" loading={joining} onClick={onJoin}>
                                {t('cluster.join')}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <FormField
                        label={t('cluster.peer_address')}
                        description={t('cluster.peer_address_hint')}
                        errorText={joinError}
                    >
                        <Input
                            value={joinAddress}
                            placeholder="10.0.0.2:4370"
                            onChange={(e) => setJoinAddress(e.detail.value)}
                        />
                    </FormField>
                </Form>
            </Modal>

            <Modal
                visible={evictTarget !== null}
                onDismiss={() => setEvictTarget(null)}
                header={t('cluster.evict_node')}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setEvictTarget(null)}>
                                {t('cluster.cancel')}
                            </Button>
                            <Button variant="primary" loading={evicting} onClick={onConfirmEvict}>
                                {t('cluster.evict')}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <SpaceBetween size="s">
                    <span>{t('cluster.evict_confirm', { id: evictTarget?.id ?? '' })}</span>
                    {evictTarget?.state === 'alive' && (
                        <Alert type="warning">{t('cluster.evict_alive_warning')}</Alert>
                    )}
                </SpaceBetween>
            </Modal>

            <Modal
                visible={linkOpen}
                onDismiss={() => setLinkOpen(false)}
                header={t('cluster.add_link')}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setLinkOpen(false)}>
                                {t('cluster.cancel')}
                            </Button>
                            <Button variant="primary" loading={savingLink} onClick={onCreateLink}>
                                {t('cluster.create')}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <SpaceBetween size="l">
                        {linkError && <Alert type="error">{linkError}</Alert>}

                        <FormField
                            label={t('cluster.link_name')}
                            description={t('cluster.link_name_hint')}
                        >
                            <Input
                                value={linkName}
                                placeholder="eu-west"
                                onChange={(e) => setLinkName(e.detail.value)}
                            />
                        </FormField>

                        <FormField
                            label={t('cluster.endpoints')}
                            description={t('cluster.endpoints_hint')}
                        >
                            <Input
                                value={linkEndpoints}
                                placeholder="mq-eu-1:4370, mq-eu-2:4370"
                                onChange={(e) => setLinkEndpoints(e.detail.value)}
                            />
                        </FormField>

                        <FormField
                            label={t('cluster.forward')}
                            description={t('cluster.forward_hint')}
                        >
                            <Input
                                value={linkForward}
                                placeholder="sensors/#"
                                onChange={(e) => setLinkForward(e.detail.value)}
                            />
                        </FormField>

                        <FormField
                            label={t('cluster.accept')}
                            description={t('cluster.accept_hint')}
                        >
                            <Input
                                value={linkAccept}
                                placeholder="cmd/eu/#"
                                onChange={(e) => setLinkAccept(e.detail.value)}
                            />
                        </FormField>

                        <FormField
                            label={t('cluster.secret')}
                            description={t('cluster.secret_hint')}
                        >
                            <Input
                                type="password"
                                value={linkSecret}
                                onChange={(e) => setLinkSecret(e.detail.value)}
                            />
                        </FormField>
                    </SpaceBetween>
                </Form>
            </Modal>

            <Modal
                visible={removeTarget !== null}
                onDismiss={() => setRemoveTarget(null)}
                header={t('cluster.remove_link')}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setRemoveTarget(null)}>
                                {t('cluster.cancel')}
                            </Button>
                            <Button
                                variant="primary"
                                loading={removingLink}
                                onClick={onConfirmRemoveLink}
                            >
                                {t('cluster.remove')}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                {t('cluster.remove_link_confirm', { name: removeTarget?.name ?? '' })}
            </Modal>
        </ContentLayout>
    );
}
