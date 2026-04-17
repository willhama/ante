import type { ThemeMode, ResolvedTheme, PageSize, PageDimensions } from '$lib/types';

/** Current file path. null means untitled/new file. */
let filePath = $state<string | null>(null);

/** Whether the document has unsaved changes. */
let isDirty = $state(false);

/** Theme mode: system, light, or dark. */
let themeMode = $state<ThemeMode>('system');

/** OS-reported preference. Updated by media query listener. */
let osPrefersDark = $state(false);

/** Resolved theme after applying system preference. */
const resolvedTheme: ResolvedTheme = $derived(
  themeMode === 'system' ? (osPrefersDark ? 'dark' : 'light') : themeMode
);

/** Page size selected by the user. */
let pageSize = $state<PageSize>('letter');

/** Whether AI autocomplete is enabled by the user. Default ON. */
let aiEnabled = $state<boolean>(true);

/** Whether the AI feature is available (API key present at startup). */
let aiAvailable = $state<boolean>(false);

/** Shared header text rendered in the top margin of every page. */
let headerText = $state<string>('');

/** Shared footer text rendered in the bottom margin of every page. */
let footerText = $state<string>('');

/** Pixel dimensions (at 96 dpi) for each supported page size. */
const PAGE_SIZE_MAP: Record<PageSize, PageDimensions> = {
  letter: { width: 816, height: 1056 },
  a4:     { width: 794, height: 1123 },
  legal:  { width: 816, height: 1344 },
};

/** Fixed margin in px (1 inch at 96 dpi). */
const PAGE_MARGIN = 96;

/** Gap between page backgrounds in px. */
const PAGE_GAP = 24;

/** Current page pixel dimensions, derived from pageSize. */
const pageDimensions: PageDimensions = $derived(PAGE_SIZE_MAP[pageSize]);

/** Page width in px. */
const pageWidth: number = $derived(pageDimensions.width);

/** Page height in px. */
const pageHeight: number = $derived(pageDimensions.height);

/** Margin in px (1 inch). */
const pageMargin: number = PAGE_MARGIN;

/** Gap between page backgrounds in px. */
const pageGap: number = PAGE_GAP;

/** Usable content width per page (pageWidth - 2 * margin). */
const contentWidth: number = $derived(pageWidth - 2 * PAGE_MARGIN);

/** Usable content height per page (pageHeight - 2 * margin). */
const contentHeightPerPage: number = $derived(pageHeight - 2 * PAGE_MARGIN);

/** Extract just the filename from a full path (handles both / and \ separators). */
function getFilename(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** Window title following the pattern: [dirty]{filename} - ante */
const windowTitle: string = $derived.by(() => {
  const name = filePath ? getFilename(filePath) : 'Untitled.html';
  const dirty = isDirty ? '* ' : '';
  return `${dirty}${name} - ante`;
});

export const appState = {
  get filePath() { return filePath; },
  set filePath(v: string | null) { filePath = v; },

  get isDirty() { return isDirty; },
  set isDirty(v: boolean) { isDirty = v; },

  get themeMode() { return themeMode; },
  set themeMode(v: ThemeMode) { themeMode = v; },

  get osPrefersDark() { return osPrefersDark; },
  set osPrefersDark(v: boolean) { osPrefersDark = v; },

  get resolvedTheme(): ResolvedTheme { return resolvedTheme; },
  get windowTitle(): string { return windowTitle; },

  get pageSize(): PageSize { return pageSize; },
  set pageSize(v: PageSize) { pageSize = v; },

  get pageWidth(): number { return pageWidth; },
  get pageHeight(): number { return pageHeight; },
  get pageMargin(): number { return pageMargin; },
  get pageGap(): number { return pageGap; },
  get contentWidth(): number { return contentWidth; },
  get contentHeightPerPage(): number { return contentHeightPerPage; },

  get aiEnabled(): boolean { return aiEnabled; },
  set aiEnabled(v: boolean) { aiEnabled = v; },

  get aiAvailable(): boolean { return aiAvailable; },
  setAiAvailable(v: boolean): void { aiAvailable = v; },

  get headerText(): string { return headerText; },
  set headerText(v: string) { headerText = v; },

  get footerText(): string { return footerText; },
  set footerText(v: string) { footerText = v; },
};
