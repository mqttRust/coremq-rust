import { useNavigate } from 'react-router-dom';

import Box from '@cloudscape-design/components/box';
import Button from '@cloudscape-design/components/button';
import SpaceBetween from '@cloudscape-design/components/space-between';

export function NotFoundView() {
    const navigate = useNavigate();

    return (
        <Box textAlign="center" padding={{ top: 'xxxl' }}>
            <SpaceBetween size="m">
                <Box variant="h1">404</Box>
                <Box variant="p" color="text-body-secondary">
                    Page not found
                </Box>
                <div>
                    <Button variant="primary" onClick={() => navigate('/')}>
                        Go to home
                    </Button>
                </div>
            </SpaceBetween>
        </Box>
    );
}

export default NotFoundView;
