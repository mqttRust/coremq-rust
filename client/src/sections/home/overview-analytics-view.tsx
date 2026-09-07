import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import ContentLayout from '@cloudscape-design/components/content-layout';
import Container from '@cloudscape-design/components/container';
import Header from '@cloudscape-design/components/header';
import ColumnLayout from '@cloudscape-design/components/column-layout';
import Box from '@cloudscape-design/components/box';
import Badge from '@cloudscape-design/components/badge';
import Spinner from '@cloudscape-design/components/spinner';
import SpaceBetween from '@cloudscape-design/components/space-between';
import StatusIndicator from '@cloudscape-design/components/status-indicator';
import Table from '@cloudscape-design/components/table';
import LineChart from '@cloudscape-design/components/line-chart';

import type { MetricsFrame } from 'src/types/metrics';
import type { TopicInfo } from 'src/types/topics';
import { useMetrics } from 'src/hooks/use-metrics';

type ChartPoint = { x: number; y: number };

function buildSeries(
    history: MetricsFrame[],
    pick: (f: MetricsFrame) => number,
): ChartPoint[] {
    return history.map((f, i) => ({ x: i, y: pick(f) }));
}

export default function HomeView() {
    const { t } = useTranslation();
    const { latest, history, status } = useMetrics();

    const liveIndicator = useMemo(() => {
        if (status === 'open') return <StatusIndicator type="success">Live</StatusIndicator>;
        if (status === 'connecting')
            return <StatusIndicator type="loading">Connecting</StatusIndicator>;
        return <StatusIndicator type="error">Disconnected</StatusIndicator>;
    }, [status]);

    const clientCount = latest?.client_count ?? 0;
    const topicCount = latest?.topics.length ?? 0;
    const totalSubscriptions = useMemo(
        () => (latest?.topics ?? []).reduce((sum, topic) => sum + topic.subscriber_count, 0),
        [latest],
    );
    const cpuPercent = latest?.cpu_percent ?? 0;
    const memoryMb = latest?.memory_mb ?? 0;

    const cpuData = useMemo(() => buildSeries(history, (f) => f.cpu_percent), [history]);
    const memoryData = useMemo(() => buildSeries(history, (f) => f.memory_mb), [history]);
    const xDomain = useMemo<[number, number]>(
        () => [0, Math.max(history.length - 1, 1)],
        [history.length],
    );

    const topTopics = useMemo<TopicInfo[]>(
        () =>
            [...(latest?.topics ?? [])].sort(
                (a, b) => b.subscriber_count - a.subscriber_count,
            ),
        [latest],
    );

    const chartStatus = history.length === 0 ? 'loading' : 'finished';
    const chartEmpty = (
        <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
            <b>No data</b>
            <Box variant="p" color="text-body-secondary">
                Waiting for live metrics.
            </Box>
        </Box>
    );

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="Real-time health of this MQTT broker, streamed live over the metrics WebSocket."
                    actions={liveIndicator}
                >
                    Dashboard
                </Header>
            }
        >
            <SpaceBetween size="l">
                <Container
                    header={
                        <Header
                            variant="h2"
                            description="Live broker metrics, refreshed once per second."
                        >
                            Broker health
                        </Header>
                    }
                >
                    {latest === null ? (
                        <Box textAlign="center" padding={{ vertical: 'xxl' }}>
                            <SpaceBetween size="s" alignItems="center">
                                <Spinner size="large" />
                                <Box variant="p" color="text-body-secondary">
                                    Connecting to the live metrics stream…
                                </Box>
                            </SpaceBetween>
                        </Box>
                    ) : (
                        <ColumnLayout columns={4} variant="text-grid">
                            <div>
                                <Box variant="awsui-key-label">Connected clients</Box>
                                <Box variant="h1">{clientCount}</Box>
                            </div>
                            <div>
                                <Box variant="awsui-key-label">Active topics</Box>
                                <Box variant="h1">{topicCount}</Box>
                            </div>
                            <div>
                                <Box variant="awsui-key-label">Total subscriptions</Box>
                                <Box variant="h1">{totalSubscriptions}</Box>
                            </div>
                            <div>
                                <Box variant="awsui-key-label">CPU usage</Box>
                                <Box variant="h1">{cpuPercent.toFixed(1)}%</Box>
                            </div>
                            <div>
                                <Box variant="awsui-key-label">Memory</Box>
                                <Box variant="h1">
                                    {memoryMb.toFixed(1)}{' '}
                                    <Box variant="span" color="text-body-secondary" fontSize="heading-m">
                                        MB
                                    </Box>
                                </Box>
                            </div>
                        </ColumnLayout>
                    )}
                </Container>

                <ColumnLayout columns={2}>
                    <Container header={<Header variant="h2">CPU usage over time</Header>}>
                        <LineChart
                            series={[
                                {
                                    title: 'CPU %',
                                    type: 'line',
                                    data: cpuData,
                                    valueFormatter: (value) => `${value.toFixed(1)}%`,
                                },
                            ]}
                            xScaleType="linear"
                            xDomain={xDomain}
                            height={220}
                            hideFilter
                            hideLegend
                            xTitle="Samples"
                            yTitle="CPU %"
                            statusType={chartStatus}
                            loadingText="Waiting for live metrics"
                            empty={chartEmpty}
                            noMatch={chartEmpty}
                        />
                    </Container>

                    <Container header={<Header variant="h2">Memory usage over time</Header>}>
                        <LineChart
                            series={[
                                {
                                    title: 'Memory MB',
                                    type: 'line',
                                    data: memoryData,
                                    valueFormatter: (value) => `${value.toFixed(1)} MB`,
                                },
                            ]}
                            xScaleType="linear"
                            xDomain={xDomain}
                            height={220}
                            hideFilter
                            hideLegend
                            xTitle="Samples"
                            yTitle="Memory (MB)"
                            statusType={chartStatus}
                            loadingText="Waiting for live metrics"
                            empty={chartEmpty}
                            noMatch={chartEmpty}
                        />
                    </Container>
                </ColumnLayout>

                <Table<TopicInfo>
                    variant="container"
                    header={
                        <Header variant="h2" counter={`(${topTopics.length})`}>
                            Top topics
                        </Header>
                    }
                    items={topTopics}
                    trackBy="topic"
                    columnDefinitions={[
                        {
                            id: 'topic',
                            header: t('topics.topic'),
                            cell: (topic) => <Box variant="samp">{topic.topic}</Box>,
                        },
                        {
                            id: 'subscribers',
                            header: t('topics.subscribers'),
                            cell: (topic) => <Badge color="blue">{topic.subscriber_count}</Badge>,
                        },
                    ]}
                    empty={
                        <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                            <SpaceBetween size="xs">
                                <b>No topics</b>
                                <span>No topics currently have subscribers.</span>
                            </SpaceBetween>
                        </Box>
                    }
                />
            </SpaceBetween>
        </ContentLayout>
    );
}
