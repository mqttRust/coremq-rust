import { useState } from 'react';

import ContentLayout from '@cloudscape-design/components/content-layout';
import Header from '@cloudscape-design/components/header';
import Container from '@cloudscape-design/components/container';
import Form from '@cloudscape-design/components/form';
import FormField from '@cloudscape-design/components/form-field';
import Input from '@cloudscape-design/components/input';
import Textarea from '@cloudscape-design/components/textarea';
import Button from '@cloudscape-design/components/button';
import SpaceBetween from '@cloudscape-design/components/space-between';
import Box from '@cloudscape-design/components/box';
import Alert from '@cloudscape-design/components/alert';
import ColumnLayout from '@cloudscape-design/components/column-layout';

import JSZip from 'jszip';

import { generateCert, type GeneratedCert } from 'src/services/tls';
import { notify } from 'src/stores/notification-store';

export function CertificatesView() {
    const [commonName, setCommonName] = useState('localhost');
    const [sans, setSans] = useState('localhost\n127.0.0.1');
    const [generating, setGenerating] = useState(false);
    const [result, setResult] = useState<GeneratedCert | null>(null);

    const onGenerate = async () => {
        const cn = commonName.trim();
        if (!cn) {
            notify('error', 'Common name is required');
            return;
        }
        const sanList = sans
            .split(/[\n,]/)
            .map((s) => s.trim())
            .filter(Boolean);

        setGenerating(true);
        try {
            const res = await generateCert({ common_name: cn, sans: sanList });
            if (res.data) {
                setResult(res.data);
                notify('success', 'Certificate generated');
            } else {
                notify('error', res.message || 'Failed to generate certificate');
            }
        } catch (err: any) {
            notify('error', err?.response?.data?.message || err?.message || 'Failed to generate certificate');
        } finally {
            setGenerating(false);
        }
    };

    const downloadZip = async () => {
        if (!result) return;
        const zip = new JSZip();
        zip.file('server.crt', result.cert);
        zip.file('server.key', result.key);
        zip.file(
            'README.txt',
            [
                'CoreMQ self-signed TLS certificate',
                '',
                `Names: ${result.names.join(', ')}`,
                '',
                'Use with a tls/wss listener:',
                '  cert: server.crt',
                '  key:  server.key',
                '',
                'You can paste the contents of these files directly into the',
                'Create Listener form (Certificate / Private key fields).',
            ].join('\n'),
        );
        const blob = await zip.generateAsync({ type: 'blob' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `coremq-tls-${result.names[0] ?? 'cert'}.zip`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    };

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="Generate a self-signed TLS certificate and key for use with tls / wss listeners, and download them as a ZIP."
                >
                    TLS Certificates
                </Header>
            }
        >
            <SpaceBetween size="l">
                <Container header={<Header variant="h2">Generate certificate</Header>}>
                    <Form
                        actions={
                            <Button variant="primary" loading={generating} onClick={onGenerate}>
                                Generate
                            </Button>
                        }
                    >
                        <SpaceBetween size="l">
                            <FormField
                                label="Common name (CN)"
                                description="Primary hostname the certificate is issued for."
                            >
                                <Input value={commonName} onChange={(e) => setCommonName(e.detail.value)} placeholder="localhost" />
                            </FormField>
                            <FormField
                                label="Subject alternative names"
                                description="One per line (or comma-separated). DNS names or IP addresses."
                            >
                                <Textarea value={sans} onChange={(e) => setSans(e.detail.value)} rows={3} />
                            </FormField>
                        </SpaceBetween>
                    </Form>
                </Container>

                {result && (
                    <Container
                        header={
                            <Header
                                variant="h2"
                                actions={
                                    <Button variant="primary" onClick={downloadZip}>
                                        Download ZIP
                                    </Button>
                                }
                            >
                                Generated material
                            </Header>
                        }
                    >
                        <SpaceBetween size="m">
                            <Alert type="warning" header="Self-signed">
                                This certificate is self-signed — clients must trust it explicitly (or disable
                                verification). Keep <b>server.key</b> secret.
                            </Alert>
                            <Box>Names: {result.names.join(', ')}</Box>
                            <ColumnLayout columns={2}>
                                <FormField label="Certificate (server.crt)">
                                    <Textarea value={result.cert} readOnly rows={10} />
                                </FormField>
                                <FormField label="Private key (server.key)">
                                    <Textarea value={result.key} readOnly rows={10} />
                                </FormField>
                            </ColumnLayout>
                        </SpaceBetween>
                    </Container>
                )}
            </SpaceBetween>
        </ContentLayout>
    );
}

export default CertificatesView;
