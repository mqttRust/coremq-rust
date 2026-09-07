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
import TextFilter from '@cloudscape-design/components/text-filter';

import { useUserStore, selectUsers } from 'src/stores/user-store';
import { notify } from 'src/stores/notification-store';

type User = { username: string; password_hash: string; role: string };

const ROLE_OPTIONS = [
    { label: 'Admin', value: 'admin' },
    { label: 'User', value: 'user' },
];

export function AdminView() {
    const { t } = useTranslation();
    const users = useUserStore(selectUsers);
    const loading = useUserStore((s) => s.loading);
    const fetch = useUserStore((s) => s.fetch);
    const create = useUserStore((s) => s.create);

    const [filterText, setFilterText] = useState('');

    const [modalOpen, setModalOpen] = useState(false);
    const [creating, setCreating] = useState(false);
    const [newUsername, setNewUsername] = useState('');
    const [newPassword, setNewPassword] = useState('');
    const [newRole, setNewRole] = useState(ROLE_OPTIONS[1]);
    const [usernameError, setUsernameError] = useState('');
    const [passwordError, setPasswordError] = useState('');

    useEffect(() => {
        fetch();
    }, [fetch]);

    const items = useMemo(() => {
        const q = filterText.trim().toLowerCase();
        if (!q) return users;
        return users.filter((u) => u.username.toLowerCase().includes(q));
    }, [users, filterText]);

    const resetForm = () => {
        setNewUsername('');
        setNewPassword('');
        setNewRole(ROLE_OPTIONS[1]);
        setUsernameError('');
        setPasswordError('');
    };

    const openModal = () => {
        resetForm();
        setModalOpen(true);
    };

    const closeModal = () => {
        setModalOpen(false);
        resetForm();
    };

    const handleCreate = async () => {
        const username = newUsername.trim();
        const password = newPassword.trim();

        let invalid = false;
        if (!username) {
            setUsernameError(t('admin.validationRequired'));
            invalid = true;
        } else {
            setUsernameError('');
        }
        if (!password) {
            setPasswordError(t('admin.validationRequired'));
            invalid = true;
        } else {
            setPasswordError('');
        }
        if (invalid) return;

        setCreating(true);

        // Backend quirk: create() may resolve/throw even when the request
        // actually succeeded (the success path can surface as a 500-looking
        // response). So we treat both the boolean result AND the presence of
        // the user after the store's refetch as success signals.
        let result = false;
        try {
            result = await create({
                username,
                password_hash: password, // PLAINTEXT; backend hashes it
                role: newRole.value!,
            });
        } catch {
            result = false;
        }

        setCreating(false);

        const state = useUserStore.getState();
        const created = state.users.some((u) => u.username === username);

        if (result || created) {
            notify('success', `User ${username} created`);
            closeModal();
        } else {
            notify('error', state.error ?? 'Failed to create user');
        }
    };

    return (
        <ContentLayout
            header={
                <Header
                    variant="h1"
                    description="Broker users and their access roles. Admins can manage the broker; users have limited access."
                    counter={`(${users.length})`}
                    actions={
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="primary" onClick={openModal}>
                                {t('admin.addUser')}
                            </Button>
                            <Button onClick={() => fetch()} loading={loading}>
                                Refresh
                            </Button>
                        </SpaceBetween>
                    }
                >
                    {t('admin.title')}
                </Header>
            }
        >
            <Table<User>
                variant="container"
                loading={loading}
                loadingText="Loading users"
                items={items}
                trackBy="username"
                filter={
                    <TextFilter
                        filteringText={filterText}
                        filteringPlaceholder="Find users"
                        onChange={(e) => setFilterText(e.detail.filteringText)}
                    />
                }
                columnDefinitions={[
                    {
                        id: 'index',
                        header: '#',
                        cell: (u) => items.indexOf(u) + 1,
                        width: 60,
                    },
                    {
                        id: 'username',
                        header: t('admin.username'),
                        cell: (u) => <Box fontWeight="bold">{u.username}</Box>,
                        sortingField: 'username',
                    },
                    {
                        id: 'role',
                        header: t('admin.role'),
                        cell: (u) => (
                            <Badge color={u.role === 'admin' ? 'green' : 'grey'}>
                                {u.role.toUpperCase()}
                            </Badge>
                        ),
                        sortingField: 'role',
                    },
                ]}
                empty={
                    <Box textAlign="center" color="inherit" padding={{ vertical: 'l' }}>
                        <SpaceBetween size="xs">
                            <b>{t('admin.empty')}</b>
                            <span>No users have been created yet.</span>
                        </SpaceBetween>
                    </Box>
                }
            />

            <Modal
                visible={modalOpen}
                onDismiss={closeModal}
                header={t('admin.addUser')}
                footer={
                    <Box float="right">
                        <SpaceBetween direction="horizontal" size="xs">
                            <Button variant="link" onClick={closeModal}>
                                {t('admin.cancel')}
                            </Button>
                            <Button variant="primary" loading={creating} onClick={handleCreate}>
                                {creating ? t('admin.creating') : t('admin.create')}
                            </Button>
                        </SpaceBetween>
                    </Box>
                }
            >
                <Form>
                    <SpaceBetween size="l">
                        <FormField label={t('admin.username')} errorText={usernameError}>
                            <Input
                                value={newUsername}
                                onChange={(e) => setNewUsername(e.detail.value)}
                                placeholder={t('admin.username')}
                            />
                        </FormField>
                        <FormField label={t('admin.password')} errorText={passwordError}>
                            <Input
                                type="password"
                                value={newPassword}
                                onChange={(e) => setNewPassword(e.detail.value)}
                                placeholder={t('admin.password')}
                            />
                        </FormField>
                        <FormField label={t('admin.role')}>
                            <Select
                                selectedOption={newRole}
                                options={ROLE_OPTIONS}
                                onChange={(e) => setNewRole(e.detail.selectedOption as typeof ROLE_OPTIONS[number])}
                            />
                        </FormField>
                    </SpaceBetween>
                </Form>
            </Modal>
        </ContentLayout>
    );
}

export default AdminView;
