import { Outlet } from 'react-router-dom';

import Box from '@cloudscape-design/components/box';

/** Centered, full-viewport shell for the sign-in page. */
export default function AuthShell() {
    return (
        <div
            style={{
                minHeight: '100vh',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                padding: '24px',
                background:
                    'radial-gradient(1200px 600px at 50% -10%, rgba(0,115,153,0.25), transparent), var(--color-background-layout-main, #0f1b2d)',
            }}
        >
            <Box>
                <div style={{ width: 'min(420px, 92vw)' }}>
                    <Outlet />
                </div>
            </Box>
        </div>
    );
}
