import '@cloudscape-design/global-styles/index.css';
import 'src/global.css';

import { useEffect } from 'react';

import { applyMode, applyDensity, Density } from '@cloudscape-design/global-styles';
import { applyTheme } from '@cloudscape-design/components/theming';

import { usePathname } from 'src/routes/hooks';
import { getColorMode } from 'src/utils/color-mode';

type AppProps = {
    children: React.ReactNode;
};

/** Apply the persisted (or default dark) Cloudscape color mode before first paint. */
applyMode(getColorMode());
applyDensity(Density.Comfortable);

/** Use a clean white page background in light mode (Cloudscape defaults to grey). */
applyTheme({
    theme: {
        tokens: {
            colorBackgroundLayoutMain: { light: '#ffffff', dark: '#0f1b2d' },
        },
    },
});

export default function App({ children }: AppProps) {
    useScrollToTop();

    return <>{children}</>;
}

function useScrollToTop() {
    const pathname = usePathname();

    useEffect(() => {
        window.scrollTo(0, 0);
    }, [pathname]);

    return null;
}
