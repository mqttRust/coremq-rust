import { useRouteError, isRouteErrorResponse } from 'react-router';

/** Framework-light route error screen (no UI-kit dependency). */
export function ErrorBoundary() {
    const error = useRouteError();

    return (
        <div style={rootStyle}>
            <div style={containerStyle}>{renderErrorMessage(error)}</div>
        </div>
    );
}

function parseStackTrace(stack?: string) {
    if (!stack) return { filePath: null as string | null, functionName: null as string | null };
    const filePathMatch = stack.match(/\/src\/[^?]+/);
    const functionNameMatch = stack.match(/at (\S+)/);
    return {
        filePath: filePathMatch ? filePathMatch[0] : null,
        functionName: functionNameMatch ? functionNameMatch[1] : null,
    };
}

function renderErrorMessage(error: unknown) {
    if (isRouteErrorResponse(error)) {
        return (
            <>
                <h1 style={titleStyle}>
                    {error.status}: {error.statusText}
                </h1>
                <p style={messageStyle}>{String(error.data)}</p>
            </>
        );
    }

    if (error instanceof Error) {
        const { filePath, functionName } = parseStackTrace(error.stack);
        return (
            <>
                <h1 style={titleStyle}>Unexpected Application Error!</h1>
                <p style={messageStyle}>
                    {error.name}: {error.message}
                </p>
                <pre style={detailsStyle}>{error.stack}</pre>
                {(filePath || functionName) && (
                    <p style={filePathStyle}>
                        {filePath} ({functionName})
                    </p>
                )}
            </>
        );
    }

    return <h1 style={titleStyle}>Unknown Error</h1>;
}

const MONO = '"SFMono-Regular", Consolas, "Liberation Mono", Menlo, Courier, monospace';

const rootStyle: React.CSSProperties = {
    display: 'flex',
    flex: '1 1 auto',
    alignItems: 'center',
    padding: '10vh 15px 0',
    flexDirection: 'column',
    minHeight: '100vh',
    color: 'white',
    backgroundColor: '#2c2c2e',
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Arial, sans-serif',
};

const containerStyle: React.CSSProperties = {
    gap: 24,
    padding: 20,
    width: '100%',
    maxWidth: 960,
    display: 'flex',
    borderRadius: 8,
    flexDirection: 'column',
    backgroundColor: '#1c1c1e',
};

const titleStyle: React.CSSProperties = { margin: 0, lineHeight: 1.2, fontSize: 20, fontWeight: 700 };

const messageStyle: React.CSSProperties = {
    margin: 0,
    lineHeight: 1.5,
    padding: '12px 16px',
    whiteSpace: 'pre-wrap',
    color: '#ff5555',
    fontSize: 14,
    fontFamily: MONO,
    backgroundColor: '#2a1e1e',
    borderLeft: '2px solid #ff5555',
    fontWeight: 700,
};

const detailsStyle: React.CSSProperties = {
    margin: 0,
    padding: 16,
    lineHeight: 1.5,
    overflow: 'auto',
    borderRadius: 8,
    color: '#e2aa53',
    backgroundColor: '#111111',
    fontFamily: MONO,
};

const filePathStyle: React.CSSProperties = { marginTop: 0, color: '#2dd9da' };
