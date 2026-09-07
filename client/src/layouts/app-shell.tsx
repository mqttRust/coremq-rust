import { useMemo, useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import Cookies from 'js-cookie';

import AppLayout from '@cloudscape-design/components/app-layout';
import TopNavigation from '@cloudscape-design/components/top-navigation';
import SideNavigation, { type SideNavigationProps } from '@cloudscape-design/components/side-navigation';
import BreadcrumbGroup from '@cloudscape-design/components/breadcrumb-group';
import Flashbar from '@cloudscape-design/components/flashbar';
import { applyMode, Mode } from '@cloudscape-design/global-styles';

import i18n from 'src/118n/index';
import { getColorMode, setColorMode } from 'src/utils/color-mode';
import { useNotificationStore } from 'src/stores/notification-store';

const LANGS = [
    { id: 'en', text: 'English' },
    { id: 'ko', text: '한국어' },
    { id: 'uz', text: "O'zbek" },
];

/** Map every route path to its breadcrumb / title text. */
function usePageMeta() {
    const { t } = useTranslation();
    return useMemo(
        () => ({
            '/': t('nav.home'),
            '/sessions': t('nav.sessions'),
            '/topics': t('nav.topics'),
            '/listeners': t('nav.listeners'),
            '/websockets': t('nav.websocket'),
            '/webhooks': t('nav.webhook'),
            '/certificates': 'Certificates',
            '/authentication': 'Authentication',
            '/cluster': t('nav.cluster'),
            '/admins': t('nav.admin'),
        }) as Record<string, string>,
        [t],
    );
}

export default function AppShell() {
    const navigate = useNavigate();
    const location = useLocation();
    const { t } = useTranslation();
    const meta = usePageMeta();

    const [navOpen, setNavOpen] = useState(true);
    const [mode, setMode] = useState<Mode>(getColorMode());
    const [lang, setLang] = useState(i18n.language);

    const flashItems = useNotificationStore((s) => s.items);

    const navItems: SideNavigationProps.Item[] = useMemo(
        () => [
            { type: 'link', text: t('nav.home'), href: '/' },
            {
                type: 'section-group',
                title: 'Monitoring',
                items: [
                    { type: 'link', text: t('nav.sessions'), href: '/sessions' },
                    { type: 'link', text: t('nav.topics'), href: '/topics' },
                    { type: 'link', text: t('nav.listeners'), href: '/listeners' },
                ],
            },
            {
                type: 'section-group',
                title: 'Tools',
                items: [
                    { type: 'link', text: t('nav.websocket'), href: '/websockets' },
                    { type: 'link', text: t('nav.webhook'), href: '/webhooks' },
                    { type: 'link', text: 'Certificates', href: '/certificates' },
                ],
            },
            {
                type: 'section-group',
                title: 'Settings',
                items: [
                    { type: 'link', text: 'Authentication', href: '/authentication' },
                    { type: 'link', text: t('nav.cluster'), href: '/cluster' },
                    { type: 'link', text: t('nav.admin'), href: '/admins' },
                ],
            },
        ],
        [t],
    );

    const activeHref = location.pathname === '/' ? '/' : `/${location.pathname.split('/')[1]}`;
    const currentTitle = meta[activeHref] ?? 'CoreMQ';

    const toggleMode = () => {
        const next = mode === Mode.Dark ? Mode.Light : Mode.Dark;
        applyMode(next);
        setColorMode(next);
        setMode(next);
    };

    const changeLang = (id: string) => {
        i18n.changeLanguage(id);
        setLang(id);
    };

    const logout = () => {
        Cookies.remove('access_token', { path: '/' });
        Cookies.remove('refresh_token', { path: '/' });
        navigate('/sign-in', { replace: true });
    };

    return (
        <>
            <div id="top-nav" style={{ position: 'sticky', top: 0, zIndex: 1002 }}>
                <TopNavigation
                    identity={{
                        href: '/',
                        title: 'CoreMQ',
                        logo: { src: '/favicon.ico', alt: 'CoreMQ' },
                        onFollow: (e) => {
                            e.preventDefault();
                            navigate('/');
                        },
                    }}
                    utilities={[
                        {
                            type: 'button',
                            text: mode === Mode.Dark ? 'Light' : 'Dark',
                            ariaLabel: 'Toggle color mode',
                            onClick: toggleMode,
                        },
                        {
                            type: 'menu-dropdown',
                            ariaLabel: 'Language',
                            text: LANGS.find((l) => l.id === lang)?.text ?? 'English',
                            items: LANGS.map((l) => ({ id: l.id, text: l.text })),
                            onItemClick: (e) => changeLang(e.detail.id),
                        },
                        {
                            type: 'menu-dropdown',
                            text: 'admin',
                            description: 'admin@coremq',
                            items: [{ id: 'signout', text: t('logout') }],
                            onItemClick: (e) => {
                                if (e.detail.id === 'signout') logout();
                            },
                        },
                    ]}
                />
            </div>
            <AppLayout
                headerSelector="#top-nav"
                navigationOpen={navOpen}
                onNavigationChange={(e) => setNavOpen(e.detail.open)}
                toolsHide
                notifications={<Flashbar items={flashItems} stackItems />}
                breadcrumbs={
                    <BreadcrumbGroup
                        items={[
                            { text: 'CoreMQ', href: '/' },
                            { text: currentTitle, href: activeHref },
                        ]}
                        onFollow={(e) => {
                            e.preventDefault();
                            navigate(e.detail.href);
                        }}
                    />
                }
                navigation={
                    <SideNavigation
                        activeHref={activeHref}
                        header={{ href: '/', text: 'CoreMQ Broker' }}
                        items={navItems}
                        onFollow={(e) => {
                            if (!e.detail.external) {
                                e.preventDefault();
                                navigate(e.detail.href);
                            }
                        }}
                    />
                }
                content={<Outlet />}
            />
        </>
    );
}
