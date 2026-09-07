import { useEffect, useMemo, useState } from 'react';

import ContentLayout from '@cloudscape-design/components/content-layout';
import Header from '@cloudscape-design/components/header';
import Container from '@cloudscape-design/components/container';
import Form from '@cloudscape-design/components/form';
import FormField from '@cloudscape-design/components/form-field';
import Input from '@cloudscape-design/components/input';
import Toggle from '@cloudscape-design/components/toggle';
import Table from '@cloudscape-design/components/table';
import Button from '@cloudscape-design/components/button';
import Modal from '@cloudscape-design/components/modal';
import Tiles from '@cloudscape-design/components/tiles';
import Alert from '@cloudscape-design/components/alert';
import SpaceBetween from '@cloudscape-design/components/space-between';
import Box from '@cloudscape-design/components/box';

import {
    getAuthConfig,
    updateAuthConfig,
    listCredentials,
    createCredential,
    deleteCredential,
} from 'src/services/mqtt-auth';
import { DEFAULT_AUTH_CONFIG, type AuthConfig } from 'src/types/mqtt-auth';
import { notify } from 'src/stores/notification-store';

type Credential = { username: string };

type AuthType = 'builtin' | 'http' | 'jwt';

const AUTH_TYPES: { value: AuthType; label: string; description: string }[] = [
    {
        value: 'builtin',
        label: 'Built-in database',
        description: 'Authenticate against the credentials managed on this page.',
    },
    {
        value: 'http',
        label: 'HTTP endpoint',
        description: 'POST {clientid, username, password, peerhost}; 2xx = allow, 4xx = deny.',
    },
    {
        value: 'jwt',
        label: 'JWT',
        description: 'HS256 secret; the client password must be a valid JWT.',
    },
];

const typeLabel = (t: AuthType) => AUTH_TYPES.find((a) => a.value === t)!.label;

/** The single enabled authenticator, or null when none is enabled. */
function activeType(cfg: AuthConfig): AuthType | null {
    if (cfg.builtin_enabled) return 'builtin';
    if (cfg.http_enabled) return 'http';
    if (cfg.jwt_enabled) return 'jwt';
    return null;
}

/** Enable exactly one authenticator (or none), clearing the config of the others. */
function withAuthenticator(cfg: AuthConfig, type: AuthType | null, http_url = '', jwt_secret = ''): AuthConfig {
    return {
        ...cfg,
        builtin_enabled: type === 'builtin',
        http_enabled: type === 'http',
        http_url: type === 'http' ? http_url : '',
        jwt_enabled: type === 'jwt',
        jwt_secret: type === 'jwt' ? jwt_secret : '',
    };
}

export function AuthenticationView() {
    /** Settings state */
    const [config, setConfig] = useState<AuthConfig>(DEFAULT_AUTH_CONFIG);
    const [loadingConfig, setLoadingConfig] = useState(false);
    const [saving, setSaving] = useState(false);

    /** Credentials state */
    const [credentials, setCredentials] = useState<Credential[]>([]);
    const [credLoading, setCredLoading] = useState(false);

    /** Authenticator create/edit modal */
    const [formOpen, setFormOpen] = useState(false);
    const [formMode, setFormMode] = useState<'create' | 'edit'>('create');
    const [formType, setFormType] = useState<AuthType>('builtin');
    const [formHttpUrl, setFormHttpUrl] = useState('');
    const [formJwtSecret, setFormJwtSecret] = useState('');
    const [formError, setFormError] = useState('');
    const [submitting, setSubmitting] = useState(false);

    /** Remove authenticator confirmation */
    const [removeOpen, setRemoveOpen] = useState(false);
    const [removing, setRemoving] = useState(false);

    /** Add credential modal */
    const [addOpen, setAddOpen] = useState(false);
    const [creating, setCreating] = useState(false);
    const [newUsername, setNewUsername] = useState('');
    const [newPassword, setNewPassword] = useState('');
    const [usernameError, setUsernameError] = useState('');
    const [passwordError, setPasswordError] = useState('');

    /** Delete credential confirmation modal */
    const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
    const [deleting, setDeleting] = useState(false);

    const current = activeType(config);

    /** Legacy configs may have several authenticators enabled at once; only one is honoured here. */
    const multipleEnabled = useMemo(
        () => [config.builtin_enabled, config.http_enabled, config.jwt_enabled].filter(Boolean).length > 1,
        [config]
    );

    const loadConfig = async () => {
        setLoadingConfig(true);
        try {
            const res = await getAuthConfig();
            if (res.data) setConfig(res.data);
        } catch (err: any) {
            notify('error', err?.response?.data?.message || err?.message || 'Failed to load authentication settings');
        } finally {
            setLoadingConfig(false);
        }
    };

    const loadCredentials = async () => {
        setCredLoading(true);
        try {
            const res = await listCredentials();
            setCredentials((res.data ?? []).map((username) => ({ username })));
        } catch (err: any) {
            notify('error', err?.response?.data?.message || err?.message || 'Failed to load credentials');
        } finally {
            setCredLoading(false);
        }
    };

    useEffect(() => {
        loadConfig();
        loadCredentials();
    }, []);

    /** PUT the whole config; returns true on success. */
    const persist = async (next: AuthConfig, successMessage: string, failureMessage: string) => {
        try {
            const res = await updateAuthConfig(next);
            setConfig(res.data ?? next);
            notify('success', successMessage);
            return true;
        } catch (err: any) {
            notify('error', err?.response?.data?.message || err?.message || failureMessage);
            return false;
        }
    };

    const onSaveAnonymous = async () => {
        setSaving(true);
        await persist(config, 'Authentication settings saved', 'Failed to save authentication settings');
        setSaving(false);
    };

    /** Authenticator */

    const openCreate = () => {
        setFormMode('create');
        setFormType('builtin');
        setFormHttpUrl('');
        setFormJwtSecret('');
        setFormError('');
        setFormOpen(true);
    };

    const openEdit = () => {
        if (!current) return;
        setFormMode('edit');
        setFormType(current);
        setFormHttpUrl(config.http_url);
        setFormJwtSecret(config.jwt_secret);
        setFormError('');
        setFormOpen(true);
    };

    const onSubmitAuthenticator = async () => {
        const httpUrl = formHttpUrl.trim();
        const jwtSecret = formJwtSecret.trim();

        if (formType === 'http' && !httpUrl) {
            setFormError('HTTP endpoint URL is required');
            return;
        }
        if (formType === 'jwt' && !jwtSecret) {
            setFormError('JWT secret is required');
            return;
        }
        setFormError('');

        setSubmitting(true);
        const ok = await persist(
            withAuthenticator(config, formType, httpUrl, jwtSecret),
            formMode === 'create'
                ? `${typeLabel(formType)} authenticator created`
                : `${typeLabel(formType)} authenticator updated`,
            'Failed to save authenticator'
        );
        setSubmitting(false);
        if (ok) setFormOpen(false);
    };

    const onRemoveAuthenticator = async () => {
        setRemoving(true);
        const ok = await persist(
            withAuthenticator(config, null),
            'Authenticator removed',
            'Failed to remove authenticator'
        );
        setRemoving(false);
        if (ok) setRemoveOpen(false);
    };

    /** Credentials */

    const openAdd = () => {
        setNewUsername('');
        setNewPassword('');
        setUsernameError('');
        setPasswordError('');
        setAddOpen(true);
    };

    const closeAdd = () => {
        setAddOpen(false);
    };

    const onCreate = async () => {
        const username = newUsername.trim();
        const password = newPassword;

        let invalid = false;
        if (!username) {
            setUsernameError('Username is required');
            invalid = true;
        } else {
            setUsernameError('');
        }
        if (!password) {
            setPasswordError('Password is required');
            invalid = true;
        } else {
            setPasswordError('');
        }
        if (invalid) return;

        setCreating(true);
        try {
            await createCredential(username, password);
            notify('success', `Credential ${username} created`);
            closeAdd();
            await loadCredentials();
        } catch (err: any) {
            notify('error', err?.response?.data?.message || err?.message || 'Failed to create credential');
        } finally {
            setCreating(false);
        }
    };

    const onConfirmDelete = async () => {
        if (!deleteTarget) return;
        setDeleting(true);
        try {
            await deleteCredential(deleteTarget);
            notify('success', `Credential ${deleteTarget} deleted`);
            setDeleteTarget(null);
            await loadCredentials();
        } catch (err: any) {
            notify('error', err?.response?.data?.message || err?.message || 'Failed to delete credential');
        } finally {
            setDeleting(false);
        }
    };

    const authenticatorRows = current ? [{ type: current }] : [];

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="MQTT clients are authenticated on connect by a single authenticator: the built-in database, an HTTP endpoint, or JWT. If no authenticator is configured, the connection falls back to the allow-anonymous setting."
                >
                    Authentication
                </Header>
            }
        >
            <SpaceBetween size="l">
                {multipleEnabled && (
                    <Alert type="warning" header="Multiple authenticators enabled">
                        This broker has more than one authenticator enabled. Only one is supported here —{' '}
                        <b>{typeLabel(current!)}</b> is shown as active. Save the authenticator to disable the others.
                    </Alert>
                )}

                {/* Authenticator — at most one */}
                <Table<{ type: AuthType }>
                    variant="container"
                    loading={loadingConfig}
                    loadingText="Loading authenticator"
                    items={authenticatorRows}
                    trackBy="type"
                    header={
                        <Header
                            variant="h2"
                            description="Only one authenticator can be active at a time."
                            counter={`(${authenticatorRows.length})`}
                            actions={
                                <Button variant="primary" onClick={openCreate} disabled={current !== null}>
                                    Create authenticator
                                </Button>
                            }
                        >
                            Authenticator
                        </Header>
                    }
                    columnDefinitions={[
                        {
                            id: 'type',
                            header: 'Type',
                            cell: (r) => <Box fontWeight="bold">{typeLabel(r.type)}</Box>,
                        },
                        {
                            id: 'details',
                            header: 'Configuration',
                            cell: (r) => {
                                if (r.type === 'http') return config.http_url || '—';
                                if (r.type === 'jwt') return config.jwt_secret ? 'HS256 secret set' : 'No secret set';
                                return `${credentials.length} credential${credentials.length === 1 ? '' : 's'}`;
                            },
                        },
                        {
                            id: 'actions',
                            header: 'Actions',
                            cell: () => (
                                <SpaceBetween direction="horizontal" size="xs">
                                    <Button variant="inline-link" onClick={openEdit}>
                                        Edit
                                    </Button>
                                    <Button variant="inline-link" onClick={() => setRemoveOpen(true)}>
                                        Remove
                                    </Button>
                                </SpaceBetween>
                            ),
                        },
                    ]}
                    empty={
                        <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                            <SpaceBetween size="xs">
                                <b>No authenticator</b>
                                <span>
                                    Every client is accepted or rejected by the allow-anonymous setting alone.
                                </span>
                                <Button onClick={openCreate}>Create authenticator</Button>
                            </SpaceBetween>
                        </Box>
                    }
                />

                {/* Anonymous fallback */}
                <Container header={<Header variant="h2">Authentication settings</Header>}>
                    <Form
                        actions={
                            <Button variant="primary" loading={saving} onClick={onSaveAnonymous}>
                                Save
                            </Button>
                        }
                    >
                        <FormField
                            label="Allow anonymous"
                            description="When no authenticator makes a decision, allow the client."
                        >
                            <Toggle
                                checked={config.allow_anonymous}
                                onChange={(e) => setConfig({ ...config, allow_anonymous: e.detail.checked })}
                            >
                                {config.allow_anonymous ? 'Enabled' : 'Disabled'}
                            </Toggle>
                        </FormField>
                    </Form>
                </Container>

                {/* Built-in credentials */}
                <Table<Credential>
                    variant="container"
                    loading={credLoading}
                    loadingText="Loading credentials"
                    items={credentials}
                    trackBy="username"
                    header={
                        <Header
                            variant="h2"
                            counter={`(${credentials.length})`}
                            description="Used by the built-in database authenticator."
                            actions={
                                <SpaceBetween direction="horizontal" size="xs">
                                    <Button onClick={() => loadCredentials()} loading={credLoading}>
                                        Refresh
                                    </Button>
                                    <Button variant="primary" onClick={openAdd}>
                                        Add credential
                                    </Button>
                                </SpaceBetween>
                            }
                        >
                            Built-in credentials
                        </Header>
                    }
                    columnDefinitions={[
                        {
                            id: 'index',
                            header: '#',
                            cell: (c) => credentials.indexOf(c) + 1,
                            width: 60,
                        },
                        {
                            id: 'username',
                            header: 'Username',
                            cell: (c) => <Box fontWeight="bold">{c.username}</Box>,
                            sortingField: 'username',
                        },
                        {
                            id: 'actions',
                            header: 'Actions',
                            cell: (c) => (
                                <Button variant="inline-link" onClick={() => setDeleteTarget(c.username)}>
                                    Delete
                                </Button>
                            ),
                        },
                    ]}
                    empty={
                        <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                            <SpaceBetween size="xs">
                                <b>No credentials</b>
                                <span>No built-in credentials have been created yet.</span>
                            </SpaceBetween>
                        </Box>
                    }
                />
            </SpaceBetween>

            {/* Create / edit authenticator modal */}
            <Modal
                visible={formOpen}
                onDismiss={() => setFormOpen(false)}
                header={formMode === 'create' ? 'Create authenticator' : 'Edit authenticator'}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setFormOpen(false)}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={submitting} onClick={onSubmitAuthenticator}>
                                {formMode === 'create' ? 'Create' : 'Save'}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <SpaceBetween size="l">
                        <FormField
                            label="Authenticator type"
                            description="Pick one. Enabling an authenticator replaces any other."
                        >
                            <Tiles
                                value={formType}
                                readOnly={formMode === 'edit'}
                                onChange={(e) => {
                                    setFormType(e.detail.value as AuthType);
                                    setFormError('');
                                }}
                                items={AUTH_TYPES.map((a) => ({
                                    value: a.value,
                                    label: a.label,
                                    description: a.description,
                                }))}
                            />
                        </FormField>

                        {formType === 'builtin' && (
                            <Box variant="p" color="text-body-secondary">
                                Clients authenticate with the usernames and passwords listed under Built-in
                                credentials.
                            </Box>
                        )}

                        {formType === 'http' && (
                            <FormField label="HTTP endpoint URL" errorText={formError}>
                                <Input
                                    value={formHttpUrl}
                                    placeholder="https://auth.example.com/mqtt"
                                    onChange={(e) => setFormHttpUrl(e.detail.value)}
                                />
                            </FormField>
                        )}

                        {formType === 'jwt' && (
                            <FormField label="JWT secret" errorText={formError}>
                                <Input
                                    type="password"
                                    value={formJwtSecret}
                                    placeholder="HS256 shared secret"
                                    onChange={(e) => setFormJwtSecret(e.detail.value)}
                                />
                            </FormField>
                        )}
                    </SpaceBetween>
                </Form>
            </Modal>

            {/* Remove authenticator confirmation */}
            <Modal
                visible={removeOpen}
                onDismiss={() => setRemoveOpen(false)}
                header="Remove authenticator"
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setRemoveOpen(false)}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={removing} onClick={onRemoveAuthenticator}>
                                Remove
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                Remove the <b>{current ? typeLabel(current) : ''}</b> authenticator? Clients will then be accepted or
                rejected by the allow-anonymous setting alone.
            </Modal>

            {/* Add credential modal */}
            <Modal
                visible={addOpen}
                onDismiss={closeAdd}
                header="Add credential"
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={closeAdd}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={creating} onClick={onCreate}>
                                Create
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <SpaceBetween size="l">
                        <FormField label="Username" errorText={usernameError}>
                            <Input
                                value={newUsername}
                                onChange={(e) => setNewUsername(e.detail.value)}
                                placeholder="Username"
                            />
                        </FormField>
                        <FormField label="Password" errorText={passwordError}>
                            <Input
                                type="password"
                                value={newPassword}
                                onChange={(e) => setNewPassword(e.detail.value)}
                                placeholder="Password"
                            />
                        </FormField>
                    </SpaceBetween>
                </Form>
            </Modal>

            {/* Delete confirmation modal */}
            <Modal
                visible={deleteTarget !== null}
                onDismiss={() => setDeleteTarget(null)}
                header="Delete credential"
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={() => setDeleteTarget(null)}>
                                Cancel
                            </Button>
                            <Button variant="primary" loading={deleting} onClick={onConfirmDelete}>
                                Delete
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                Delete credential <b>{deleteTarget}</b>? This client will no longer be able to authenticate against the
                built-in database.
            </Modal>
        </ContentLayout>
    );
}

export default AuthenticationView;
