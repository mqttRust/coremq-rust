import type { RouteObject } from 'react-router';

import { lazy, Suspense } from 'react';

import Spinner from '@cloudscape-design/components/spinner';
import Box from '@cloudscape-design/components/box';

import AppShell from 'src/layouts/app-shell';
import AuthShell from 'src/layouts/auth-shell';
import ProtectedRoute from './protected_route';

export const DashboardPage = lazy(() => import('src/pages/home'));
export const AdminPage = lazy(() => import('src/pages/admin'));
export const SessionPage = lazy(() => import('src/pages/session'));
export const SignInPage = lazy(() => import('src/pages/sign-in'));
export const ListenerPage = lazy(() => import('src/pages/listener'));
export const WebhookPage = lazy(() => import('src/pages/webhook'));
export const WebsocketPage = lazy(() => import('src/pages/websocket'));
export const TopicsPage = lazy(() => import('src/pages/topics'));
export const CertificatesPage = lazy(() => import('src/pages/certificates'));
export const AuthenticationPage = lazy(() => import('src/pages/authentication'));
export const ClusterPage = lazy(() => import('src/pages/cluster'));
export const Page404 = lazy(() => import('src/pages/page-not-found'));

const renderFallback = () => (
    <Box textAlign="center" padding={{ top: 'xxxl' }}>
        <Spinner size="large" />
    </Box>
);

const suspense = (node: React.ReactNode) => <Suspense fallback={renderFallback()}>{node}</Suspense>;

export const routesSection: RouteObject[] = [
    {
        element: <ProtectedRoute />,
        children: [
            {
                element: <AppShell />,
                children: [
                    { index: true, element: suspense(<DashboardPage />) },
                    { path: 'sessions', element: suspense(<SessionPage />) },
                    { path: 'listeners', element: suspense(<ListenerPage />) },
                    { path: 'admins', element: suspense(<AdminPage />) },
                    { path: 'webhooks', element: suspense(<WebhookPage />) },
                    { path: 'websockets', element: suspense(<WebsocketPage />) },
                    { path: 'topics', element: suspense(<TopicsPage />) },
                    { path: 'certificates', element: suspense(<CertificatesPage />) },
                    { path: 'authentication', element: suspense(<AuthenticationPage />) },
                    { path: 'cluster', element: suspense(<ClusterPage />) },
                ],
            },
        ],
    },

    {
        element: <AuthShell />,
        children: [{ path: 'sign-in', element: suspense(<SignInPage />) }],
    },

    {
        path: '404',
        element: suspense(<Page404 />),
    },

    {
        path: '*',
        element: suspense(<Page404 />),
    },
];
