import { useState, useCallback } from 'react';
import { useNavigate } from 'react-router';

import Container from '@cloudscape-design/components/container';
import Header from '@cloudscape-design/components/header';
import Form from '@cloudscape-design/components/form';
import FormField from '@cloudscape-design/components/form-field';
import Input from '@cloudscape-design/components/input';
import Button from '@cloudscape-design/components/button';
import SpaceBetween from '@cloudscape-design/components/space-between';
import Alert from '@cloudscape-design/components/alert';
import Box from '@cloudscape-design/components/box';

import Cookies from 'js-cookie';

import { SignInRequest } from 'src/types/login';
import { signIn } from 'src/services/sigin_in';
import { ApiResponse } from 'src/types/api_response';

export function SignInView() {
    const navigate = useNavigate();
    const [form, setForm] = useState<SignInRequest>({ username: '', password: '' });
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleSignIn = useCallback(async () => {
        setLoading(true);
        setError(null);
        try {
            const raw = await signIn(form);
            const response = new ApiResponse(raw.data, raw.message, raw.status_code);

            if (!response.isSuccess()) {
                setError(response.message);
                return;
            }

            Cookies.set('access_token', response.data!.access_token, { path: '/', expires: 1 });
            Cookies.set('refresh_token', response.data!.refresh_token, { path: '/', expires: 7 });
            navigate('/', { replace: true });
        } catch (err) {
            setError(err instanceof Error ? err.message : 'Unexpected error occurred.');
            console.error(err);
        } finally {
            setLoading(false);
        }
    }, [form, navigate]);

    return (
        <Container
            header={
                <Header variant="h1" description="Sign in to the broker console">
                    CoreMQ
                </Header>
            }
        >
            <form
                onSubmit={(e) => {
                    e.preventDefault();
                    handleSignIn();
                }}
            >
                <Form>
                    <SpaceBetween size="l">
                        {error && (
                            <Alert type="error" header="Sign in failed">
                                {error}
                            </Alert>
                        )}

                        <FormField label="Username">
                            <Input
                                value={form.username}
                                onChange={(e) =>
                                    setForm((prev) => ({ ...prev, username: e.detail.value }))
                                }
                            />
                        </FormField>

                        <FormField label="Password">
                            <Input
                                type="password"
                                value={form.password}
                                onChange={(e) =>
                                    setForm((prev) => ({ ...prev, password: e.detail.value }))
                                }
                            />
                        </FormField>

                        <Button variant="primary" formAction="submit" loading={loading} fullWidth>
                            Sign in
                        </Button>

                        <Box color="text-body-secondary" fontSize="body-s" textAlign="center">
                            Default credentials: admin / public
                        </Box>
                    </SpaceBetween>
                </Form>
            </form>
        </Container>
    );
}

export default SignInView;
