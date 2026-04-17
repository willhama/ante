import { appState } from './app-state.svelte';
import type { ThemeMode } from '$lib/types';

let mediaQuery: MediaQueryList | null = null;

/** Initialize OS theme detection. Call once on mount. */
export function initThemeDetection() {
  if (typeof window === 'undefined') return;

  mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
  appState.osPrefersDark = mediaQuery.matches;

  mediaQuery.addEventListener('change', handleChange);
}

/** Cleanup the media query listener. Call on unmount. */
export function destroyThemeDetection() {
  if (mediaQuery) {
    mediaQuery.removeEventListener('change', handleChange);
    mediaQuery = null;
  }
}

function handleChange(e: MediaQueryListEvent) {
  appState.osPrefersDark = e.matches;
}

/** Apply the resolved theme to the document root element. */
export function applyThemeToDocument(theme: 'light' | 'dark') {
  if (typeof document === 'undefined') return;
  document.documentElement.setAttribute('data-theme', theme);
}

/** Cycle theme mode: system -> light -> dark -> system. */
export function cycleTheme() {
  const order: ThemeMode[] = ['system', 'light', 'dark'];
  const idx = order.indexOf(appState.themeMode);
  appState.themeMode = order[(idx + 1) % order.length];
}

/** Set theme mode directly. */
function setThemeMode(mode: ThemeMode) {
  appState.themeMode = mode;
}
