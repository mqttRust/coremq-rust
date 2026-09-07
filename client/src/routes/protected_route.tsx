import { useEffect, useState } from 'react';
import { Navigate, Outlet } from 'react-router-dom';
import Cookies from 'js-cookie';

import Box from '@cloudscape-design/components/box';
import Spinner from '@cloudscape-design/components/spinner';

export default function ProtectedRoute() {
    const [loading, setLoading] = useState(true);
    const [authenticated, setAuthenticated] = useState(false);

    useEffect(() => {
        setAuthenticated(Boolean(Cookies.get('access_token')));
        setLoading(false);
    }, []);

    if (loading) {
        return (
            <Box textAlign="center" padding={{ top: 'xxxl' }}>
                <Spinner size="large" />
            </Box>
        );
    }

    return authenticated ? <Outlet /> : <Navigate to="/sign-in" replace />;
}
