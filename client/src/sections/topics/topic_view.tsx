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
import Textarea from '@cloudscape-design/components/textarea';
import Select from '@cloudscape-design/components/select';
import Toggle from '@cloudscape-design/components/toggle';
import TextFilter from '@cloudscape-design/components/text-filter';

import { useShallow } from 'zustand/react/shallow';

import type { TopicInfo } from 'src/types/topics';
import { useTopicStore, selectTopics, selectTopicStats, selectPublishHistory } from 'src/stores/topic-store';
import { publishMessage } from 'src/services/topics';
import { notify } from 'src/stores/notification-store';

type QosOption = { label: string; value: string };

const QOS_OPTIONS: QosOption[] = [
    { label: 'QoS 0', value: '0' },
    { label: 'QoS 1', value: '1' },
    { label: 'QoS 2', value: '2' },
];

export function TopicView() {
    const { t } = useTranslation();
    const topics = useTopicStore(selectTopics);
    const stats = useTopicStore(useShallow(selectTopicStats));
    const loading = useTopicStore((s) => s.loading);
    const error = useTopicStore((s) => s.error);
    const fetch = useTopicStore((s) => s.fetch);
    const publishHistory = useTopicStore(selectPublishHistory);
    const addPublish = useTopicStore((s) => s.addPublish);
    const clearPublishHistory = useTopicStore((s) => s.clearPublishHistory);

    const [filterText, setFilterText] = useState('');

    // Publish modal state
    const [modalOpen, setModalOpen] = useState(false);
    const [topicValue, setTopicValue] = useState('');
    const [payload, setPayload] = useState('');
    const [qos, setQos] = useState<QosOption>(QOS_OPTIONS[0]);
    const [retain, setRetain] = useState(false);
    const [publishing, setPublishing] = useState(false);
    const [topicError, setTopicError] = useState<string | null>(null);

    useEffect(() => {
        fetch();
    }, [fetch]);

    useEffect(() => {
        if (error) notify('error', error, t('topics.title'));
    }, [error, t]);

    const items = useMemo(() => {
        const q = filterText.trim().toLowerCase();
        if (!q) return topics;
        return topics.filter((tp) => tp.topic.toLowerCase().includes(q));
    }, [topics, filterText]);

    const openPublish = (topic = '') => {
        setTopicValue(topic);
        setPayload('');
        setQos(QOS_OPTIONS[0]);
        setRetain(false);
        setTopicError(null);
        setModalOpen(true);
    };

    const closePublish = () => {
        setModalOpen(false);
    };

    const handlePublish = async () => {
        const trimmed = topicValue.trim();
        if (!trimmed) {
            setTopicError(t('admin.validationRequired'));
            return;
        }
        setTopicError(null);
        setPublishing(true);
        const qosNum = Number(qos.value);
        try {
            await publishMessage({ topic: trimmed, payload, qos: qosNum, retain });
            addPublish({ topic: trimmed, payload, qos: qosNum, retain, ok: true });
            notify('success', `Message published to ${trimmed}`);
            // Keep the modal open so more messages can be sent; clear only the payload.
            setPayload('');
            fetch();
        } catch (err: any) {
            addPublish({ topic: trimmed, payload, qos: qosNum, retain, ok: false });
            notify('error', err?.message || 'Failed to publish message');
        } finally {
            setPublishing(false);
        }
    };

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="MQTT topics with active subscriptions on this broker. Publish a message to any topic."
                    counter={`(${stats.count})`}
                    actions={
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="primary" onClick={() => openPublish()}>
                                {t('topics.publishMessage')}
                            </Button>
                            <Button onClick={() => fetch()} loading={loading}>
                                {t('topics.refresh')}
                            </Button>
                        </SpaceBetween>
                    }
                >
                    {t('topics.title')}
                </Header>
            }
        >
            <Table<TopicInfo>
                variant="container"
                loading={loading}
                loadingText="Loading topics"
                items={items}
                trackBy="topic"
                filter={
                    <TextFilter
                        filteringText={filterText}
                        filteringPlaceholder="Find topics"
                        onChange={(e) => setFilterText(e.detail.filteringText)}
                    />
                }
                columnDefinitions={[
                    {
                        id: 'topic',
                        header: t('topics.topic'),
                        cell: (tp) => <Box variant="samp">{tp.topic}</Box>,
                        sortingField: 'topic',
                    },
                    {
                        id: 'subscribers',
                        header: t('topics.subscribers'),
                        cell: (tp) => (
                            <Badge color={tp.subscriber_count > 0 ? 'blue' : 'grey'}>
                                {tp.subscriber_count}
                            </Badge>
                        ),
                        sortingField: 'subscriber_count',
                    },
                    {
                        id: 'actions',
                        header: t('topics.actions'),
                        cell: (tp) => (
                            <Button
                                variant="inline-link"
                                onClick={() => openPublish(tp.topic)}
                            >
                                {t('topics.publish')}
                            </Button>
                        ),
                    },
                ]}
                empty={
                    <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                        <SpaceBetween size="xs">
                            <b>{t('topics.empty')}</b>
                            <span>No MQTT topics currently have subscribers.</span>
                        </SpaceBetween>
                    </Box>
                }
            />

            <Modal
                visible={modalOpen}
                onDismiss={closePublish}
                header={t('topics.publishMessage')}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={closePublish}>
                                Close
                            </Button>
                            <Button variant="primary" loading={publishing} onClick={handlePublish}>
                                {publishing ? t('topics.publishing') : t('topics.publish')}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <SpaceBetween size="l">
                    <Form>
                        <SpaceBetween size="l">
                            <FormField label={t('topics.topic')} errorText={topicError ?? undefined}>
                                <Input
                                    value={topicValue}
                                    onChange={(e) => setTopicValue(e.detail.value)}
                                    placeholder="e.g. devices/sensor/temperature"
                                />
                            </FormField>

                            <FormField label={t('topics.payload')}>
                                <Textarea
                                    value={payload}
                                    onChange={(e) => setPayload(e.detail.value)}
                                    rows={5}
                                    placeholder='e.g. {"temperature": 23.5}'
                                />
                            </FormField>

                            <SpaceBetween direction="horizontal" size="l">
                                <FormField label="QoS">
                                    <Select
                                        selectedOption={qos}
                                        onChange={(e) => setQos(e.detail.selectedOption as QosOption)}
                                        options={QOS_OPTIONS}
                                    />
                                </FormField>
                                <FormField label={t('topics.retain')}>
                                    <Toggle checked={retain} onChange={(e) => setRetain(e.detail.checked)}>
                                        {t('topics.retain')}
                                    </Toggle>
                                </FormField>
                            </SpaceBetween>
                        </SpaceBetween>
                    </Form>

                    <div>
                        <Box variant="h4">
                            Sent history{' '}
                            <Box variant="span" color="text-body-secondary">
                                ({publishHistory.length})
                            </Box>
                        </Box>
                        {publishHistory.length === 0 ? (
                            <Box color="text-body-secondary" padding={{ top: 'xs' }}>
                                No messages sent yet. Published messages will appear here.
                            </Box>
                        ) : (
                            <SpaceBetween size="xs">
                                <Box textAlign="right">
                                    <Button variant="inline-link" onClick={clearPublishHistory}>
                                        Clear history
                                    </Button>
                                </Box>
                                <div style={{ maxHeight: 220, overflowY: 'auto' }}>
                                    <SpaceBetween size="xs">
                                        {publishHistory.map((rec) => (
                                            <Box key={rec.id} variant="code">
                                                <Box variant="span" color="text-body-secondary">
                                                    {rec.time}
                                                </Box>{' '}
                                                <Badge color={rec.ok ? 'green' : 'red'}>
                                                    {rec.ok ? 'sent' : 'failed'}
                                                </Badge>{' '}
                                                <Box variant="span" color="text-status-info">
                                                    {rec.topic}
                                                </Box>{' '}
                                                <Box variant="span" color="text-body-secondary">
                                                    q{rec.qos}
                                                    {rec.retain ? ' retain' : ''}
                                                </Box>{' '}
                                                <Box variant="span">{rec.payload}</Box>
                                            </Box>
                                        ))}
                                    </SpaceBetween>
                                </div>
                            </SpaceBetween>
                        )}
                    </div>
                </SpaceBetween>
            </Modal>
        </ContentLayout>
    );
}

export default TopicView;
