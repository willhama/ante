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

/** Slug of the currently active AI provider. Surfaced in toolbar tooltip. */
let aiActiveProvider = $state<string>('openai');

/** Shared header text rendered in the top margin of every page. */
let headerText = $state<string>('');

/** Shared footer text rendered in the bottom margin of every page. */
let footerText = $state<string>('');

/** Word count derived from editor plain text. Updated by +page.svelte. */
let wordCount = $state<number>(0);

/** Character count (no spaces) derived from editor plain text. Updated by +page.svelte. */
let charCount = $state<number>(0);

/** Current page number (1-based). Updated by +page.svelte from scroll position. */
let currentPage = $state<number>(1);

/** Total page count. Updated by +page.svelte. */
let totalPages = $state<number>(1);

/** Whether the settings dialog is open. */
let settingsOpen = $state<boolean>(false);

/** Trigger speed for ghost autocomplete. Controls debounce + cooldown. */
export type AiTriggerSpeed = 'eager' | 'balanced' | 'relaxed';
let aiTriggerSpeed = $state<AiTriggerSpeed>('balanced');

/** Max tokens per suggestion. Controls suggestion length. */
let aiMaxTokens = $state<number>(80);

const TRIGGER_SPEED_DEBOUNCE: Record<AiTriggerSpeed, number> = {
  eager: 300,
  balanced: 800,
  relaxed: 1500,
};
const TRIGGER_SPEED_COOLDOWN: Record<AiTriggerSpeed, number> = {
  eager: 800,
  balanced: 2000,
  relaxed: 3500,
};
const aiDebounceMs: number = $derived(TRIGGER_SPEED_DEBOUNCE[aiTriggerSpeed]);
const aiCooldownMs: number = $derived(TRIGGER_SPEED_COOLDOWN[aiTriggerSpeed]);

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

  get aiActiveProvider(): string { return aiActiveProvider; },
  set aiActiveProvider(v: string) { aiActiveProvider = v; },

  get headerText(): string { return headerText; },
  set headerText(v: string) { headerText = v; },

  get footerText(): string { return footerText; },
  set footerText(v: string) { footerText = v; },

  get wordCount(): number { return wordCount; },
  set wordCount(v: number) { wordCount = v; },

  get charCount(): number { return charCount; },
  set charCount(v: number) { charCount = v; },

  get currentPage(): number { return currentPage; },
  set currentPage(v: number) { currentPage = v; },

  get totalPages(): number { return totalPages; },
  set totalPages(v: number) { totalPages = v; },

  get settingsOpen(): boolean { return settingsOpen; },
  set settingsOpen(v: boolean) { settingsOpen = v; },

  get aiTriggerSpeed(): AiTriggerSpeed { return aiTriggerSpeed; },
  set aiTriggerSpeed(v: AiTriggerSpeed) { aiTriggerSpeed = v; },

  get aiMaxTokens(): number { return aiMaxTokens; },
  set aiMaxTokens(v: number) { aiMaxTokens = v; },

  get aiDebounceMs(): number { return aiDebounceMs; },
  get aiCooldownMs(): number { return aiCooldownMs; },
};
