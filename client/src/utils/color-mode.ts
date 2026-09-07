import { Mode } from '@cloudscape-design/global-styles';

/** localStorage key for the persisted Cloudscape color mode. */
const STORAGE_KEY = 'coremq-color-mode';

/** Read the persisted color mode, defaulting to dark to match the broker console aesthetic. */
export function getColorMode(): Mode {
    try {
        return localStorage.getItem(STORAGE_KEY) === 'light' ? Mode.Light : Mode.Dark;
    } catch {
        return Mode.Dark;
    }
}

/** Persist the chosen color mode. */
export function setColorMode(mode: Mode): void {
    try {
        localStorage.setItem(STORAGE_KEY, mode === Mode.Light ? 'light' : 'dark');
    } catch {
        /* ignore storage failures (private mode, etc.) */
    }
}
