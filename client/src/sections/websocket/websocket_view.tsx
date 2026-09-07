import { useRef, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import ContentLayout from '@cloudscape-design/components/content-layout';
import Header from '@cloudscape-design/components/header';
import Container from '@cloudscape-design/components/container';
import ColumnLayout from '@cloudscape-design/components/column-layout';
import Grid from '@cloudscape-design/components/grid';
import Box from '@cloudscape-design/components/box';
import Button from '@cloudscape-design/components/button';
import SpaceBetween from '@cloudscape-design/components/space-between';
import Modal from '@cloudscape-design/components/modal';
import Form from '@cloudscape-design/components/form';
import FormField from '@cloudscape-design/components/form-field';
import Input from '@cloudscape-design/components/input';
import Textarea from '@cloudscape-design/components/textarea';
import Select, { SelectProps } from '@cloudscape-design/components/select';
import StatusIndicator from '@cloudscape-design/components/status-indicator';

import { useMqttTesterStore, type QoS, type LogItem } from 'src/stores/mqtt-tester-store';

const PROTOCOL_OPTIONS: SelectProps.Option[] = [
    { label: 'ws', value: 'ws' },
    { label: 'wss', value: 'wss' },
];

const QOS_OPTIONS: SelectProps.Option[] = [
    { label: 'QoS 0', value: '0' },
    { label: 'QoS 1', value: '1' },
    { label: 'QoS 2', value: '2' },
];

const consoleStyle: React.CSSProperties = {
    maxHeight: 240,
    minHeight: 140,
    overflowY: 'auto',
    padding: '8px 10px',
    borderRadius: 6,
    border: '1px solid #d1d5db',
    background: 'rgba(127,127,127,0.08)',
};

export function WebsocketView() {
    const { t } = useTranslation();

    /** The whole connection lives in a module-scoped store, so it persists across navigation. */
    const s = useMqttTesterStore();
    const { connected, connecting } = s;
    const set = s.setField;

    const [connOpen, setConnOpen] = useState(false);

    const fullUrl = `${s.protocol}://${s.url}:${s.port}${s.path.startsWith('/') ? s.path : `/${s.path}`}`;

    const subConsoleRef = useRef<HTMLDivElement>(null);
    const pubConsoleRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        subConsoleRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
    }, [s.subLogs]);
    useEffect(() => {
        pubConsoleRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
    }, [s.pubLogs]);

    const doConnect = () => {
        s.connect();
        setConnOpen(false);
    };

    const connectionStatus = connected ? (
        <StatusIndicator type="success">Connected</StatusIndicator>
    ) : connecting ? (
        <StatusIndicator type="loading">Connecting</StatusIndicator>
    ) : s.lastError ? (
        <StatusIndicator type="error">Connection refused</StatusIndicator>
    ) : (
        <StatusIndicator type="stopped">Disconnected</StatusIndicator>
    );

    const renderConsole = (logs: LogItem[], ref: React.RefObject<HTMLDivElement | null>, emptyText: string) => (
        <div ref={ref} style={consoleStyle}>
            {logs.length === 0 ? (
                <Box variant="code" color="text-body-secondary">
                    {emptyText}
                </Box>
            ) : (
                <SpaceBetween size="xxs">
                    {logs.map((log, i) => (
                        <Box key={i} variant="code">
                            <Box variant="span" color="text-body-secondary">
                                {log.time}
                            </Box>{' '}
                            <Box variant="span" color="text-status-success">
                                {log.topic}
                            </Box>{' '}
                            <Box variant="span" color="text-body-secondary">
                                q{log.qos}
                            </Box>{' '}
                            <Box variant="span">{log.payload}</Box>
                        </Box>
                    ))}
                </SpaceBetween>
            )}
        </div>
    );

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="In-browser MQTT-over-WebSocket test client. The connection stays alive while you navigate the console."
                >
                    {t('websocket.title')}
                </Header>
            }
        >
            <SpaceBetween size="l">
                {/* Connection bar */}
                <Container
                    header={
                        <Header
                            variant="h2"
                            actions={
                                connected || connecting ? (
                                    <Button onClick={() => s.disconnect()}>
                                        {t('websocket.disconnect')}
                                    </Button>
                                ) : (
                                    <Button variant="primary" onClick={() => setConnOpen(true)}>
                                        {t('websocket.connect')}
                                    </Button>
                                )
                            }
                        >
                            Connection
                        </Header>
                    }
                >
                    <ColumnLayout columns={3} variant="text-grid">
                        <div>
                            <Box variant="awsui-key-label">Status</Box>
                            {connectionStatus}
                        </div>
                        <div>
                            <Box variant="awsui-key-label">Broker URL</Box>
                            <Box variant="samp">{fullUrl}</Box>
                        </div>
                        <div>
                            <Box variant="awsui-key-label">Client ID</Box>
                            <Box variant="samp">{s.clientId}</Box>
                        </div>
                    </ColumnLayout>
                </Container>

                {/* Publish (left) and Subscribe (right), side by side */}
                <Grid gridDefinition={[{ colspan: { default: 12, m: 6 } }, { colspan: { default: 12, m: 6 } }]}>
                <Container header={<Header variant="h2" description="Send a message to a topic on the broker.">{t('websocket.publish')}</Header>}>
                    <SpaceBetween size="m">
                        <ColumnLayout columns={2}>
                            <FormField label={t('websocket.topic')}>
                                <Input
                                    value={s.pubTopic}
                                    onChange={(e) => set({ pubTopic: e.detail.value })}
                                    disabled={!connected}
                                    placeholder="test/topic"
                                />
                            </FormField>
                            <FormField label={t('websocket.qos')}>
                                <Select
                                    selectedOption={QOS_OPTIONS.find((o) => o.value === String(s.pubQoS)) ?? QOS_OPTIONS[0]}
                                    options={QOS_OPTIONS}
                                    onChange={(e) => set({ pubQoS: Number(e.detail.selectedOption.value) as QoS })}
                                    disabled={!connected}
                                />
                            </FormField>
                        </ColumnLayout>
                        <FormField label={t('websocket.message')}>
                            <Textarea
                                value={s.pubMsg}
                                onChange={(e) => set({ pubMsg: e.detail.value })}
                                rows={2}
                                disabled={!connected}
                                placeholder="Message payload"
                            />
                        </FormField>
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="primary" onClick={() => s.publish()} disabled={!connected}>
                                {t('websocket.publish')}
                            </Button>
                            <Button onClick={() => s.clearPubLogs()} disabled={s.pubLogs.length === 0}>
                                {t('websocket.clear')}
                            </Button>
                        </SpaceBetween>
                        <FormField label={t('websocket.publishConsole')}>
                            {renderConsole(s.pubLogs, pubConsoleRef, 'No messages published yet')}
                        </FormField>
                    </SpaceBetween>
                </Container>

                {/* Subscribe — full width, clearly separated */}
                <Container header={<Header variant="h2" description="Subscribe to topic filters and watch incoming messages.">{t('websocket.subscribe')}</Header>}>
                    <SpaceBetween size="m">
                        <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
                            <div style={{ flex: 1, minWidth: 180 }}>
                                <FormField label={t('websocket.topic')}>
                                    <Input
                                        value={s.newSubTopic}
                                        onChange={(e) => set({ newSubTopic: e.detail.value })}
                                        placeholder="sensors/#"
                                        disabled={!connected}
                                    />
                                </FormField>
                            </div>
                            <FormField label={t('websocket.qos')}>
                                <Select
                                    selectedOption={QOS_OPTIONS.find((o) => o.value === String(s.newSubQoS)) ?? QOS_OPTIONS[0]}
                                    options={QOS_OPTIONS}
                                    onChange={(e) => set({ newSubQoS: Number(e.detail.selectedOption.value) as QoS })}
                                    disabled={!connected}
                                />
                            </FormField>
                            <Button
                                variant="primary"
                                onClick={() => s.addSub()}
                                disabled={!connected || !s.newSubTopic.trim()}
                            >
                                {t('websocket.add')}
                            </Button>
                        </div>

                        {s.subTopics.length > 0 && (
                            <SpaceBetween size="xs">
                                {s.subTopics.map((sub) => (
                                    <Box key={sub.topic}>
                                        <SpaceBetween direction="horizontal" size="xs">
                                            <Box variant="samp" padding={{ top: 'xxs' }}>
                                                {sub.topic} (QoS {sub.qos})
                                            </Box>
                                            <Button
                                                variant="inline-link"
                                                onClick={() => s.removeSub(sub.topic)}
                                                disabled={!connected}
                                            >
                                                {t('websocket.remove')}
                                            </Button>
                                        </SpaceBetween>
                                    </Box>
                                ))}
                            </SpaceBetween>
                        )}

                        <FormField
                            label={t('websocket.subscribeConsole')}
                            secondaryControl={
                                <Button onClick={() => s.clearSubLogs()} disabled={s.subLogs.length === 0}>
                                    {t('websocket.clear')}
                                </Button>
                            }
                        >
                            {renderConsole(s.subLogs, subConsoleRef, 'Waiting for messages...')}
                        </FormField>
                    </SpaceBetween>
                </Container>
                </Grid>
            </SpaceBetween>

            {/* Connection modal */}
            <Modal
                visible={connOpen}
                onDismiss={() => setConnOpen(false)}
                header="Broker connection"
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setConnOpen(false)}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={connecting} onClick={doConnect}>
                                {t('websocket.connect')}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <SpaceBetween size="l">
                        <ColumnLayout columns={2}>
                            <FormField label={t('websocket.url')}>
                                <Input value={s.url} onChange={(e) => set({ url: e.detail.value })} placeholder="localhost" />
                            </FormField>
                            <FormField label={t('websocket.port')}>
                                <Input value={s.port} onChange={(e) => set({ port: e.detail.value })} placeholder="8083" />
                            </FormField>
                            <FormField label={t('websocket.path')}>
                                <Input value={s.path} onChange={(e) => set({ path: e.detail.value })} placeholder="/mqtt" />
                            </FormField>
                            <FormField label={t('websocket.protocol')}>
                                <Select
                                    selectedOption={PROTOCOL_OPTIONS.find((o) => o.value === s.protocol) ?? PROTOCOL_OPTIONS[0]}
                                    options={PROTOCOL_OPTIONS}
                                    onChange={(e) => set({ protocol: (e.detail.selectedOption.value as 'ws' | 'wss') ?? 'ws' })}
                                />
                            </FormField>
                            <FormField label={t('websocket.username')}>
                                <Input value={s.username} onChange={(e) => set({ username: e.detail.value })} />
                            </FormField>
                            <FormField label={t('websocket.password')}>
                                <Input type="password" value={s.password} onChange={(e) => set({ password: e.detail.value })} />
                            </FormField>
                        </ColumnLayout>
                        <FormField label={t('websocket.clientId')}>
                            <Input value={s.clientId} onChange={(e) => set({ clientId: e.detail.value })} />
                        </FormField>
                        <Box variant="samp" color="text-body-secondary">
                            {fullUrl}
                        </Box>
                    </SpaceBetween>
                </Form>
            </Modal>
        </ContentLayout>
    );
}

export default WebsocketView;
