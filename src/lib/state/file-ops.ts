import { invoke } from '@tauri-apps/api/core';
import { message } from '@tauri-apps/plugin-dialog';
import type { OpenedFile, SaveAsResult, AnteError } from '$lib/types';
import { ERROR_MESSAGES } from '$lib/types';
import { appState } from './app-state.svelte';
import { recentFiles } from './recent-files.svelte';
import { docxBytesToHtml, htmlToDocxBytes } from '$lib/io/docx';

/** Snapshot of the document at last save, used for dirty detection. */
let savedSnapshot = '';

/** Max bytes we scan at the top of a file looking for ante metadata comments. */
const METADATA_SCAN_BYTES = 4096;

/** Encode plain text as base64 safely for UTF-8 content. */
function encodeMetaValue(text: string): string {
  return btoa(encodeURIComponent(text));
}

/** Decode base64 metadata back to plain text. Returns '' on any parse failure. */
function decodeMetaValue(b64: string): string {
  try {
    return decodeURIComponent(atob(b64));
  } catch {
    return '';
  }
}

/**
 * Extract header/footer from the top of a saved HTML file, and strip those
 * ante metadata comments from the returned HTML so they don't leak into the
 * editor content.
 */
export function parseHeaderFooter(html: string): {
  header: string;
  footer: string;
  stripped: string;
} {
  const scanRegion = html.slice(0, METADATA_SCAN_BYTES);
  const headerMatch = scanRegion.match(/<!--\s*ante:header=([A-Za-z0-9+/=]*)\s*-->/);
  const footerMatch = scanRegion.match(/<!--\s*ante:footer=([A-Za-z0-9+/=]*)\s*-->/);

  const header = headerMatch ? decodeMetaValue(headerMatch[1]) : '';
  const footer = footerMatch ? decodeMetaValue(footerMatch[1]) : '';

  // Strip the two comments (plus a trailing newline if present) from wherever
  // they appear in the scanned prefix. Only scan the prefix so we never touch
  // user-authored content deeper in the document.
  let stripped = html;
  const strip = (re: RegExp) => {
    stripped = stripped.replace(re, '');
  };
  strip(/^<!--\s*ante:header=[A-Za-z0-9+/=]*\s*-->\n?/);
  strip(/^<!--\s*ante:footer=[A-Za-z0-9+/=]*\s*-->\n?/);
  // Handle case where footer comes first (tolerant).
  strip(/^<!--\s*ante:header=[A-Za-z0-9+/=]*\s*-->\n?/);

  return { header, footer, stripped };
}

/** Prepend ante:header / ante:footer comments to an HTML payload for saving. */
export function serializeWithHeaderFooter(html: string, header: string, footer: string): string {
  const headerComment = `<!-- ante:header=${encodeMetaValue(header)} -->`;
  const footerComment = `<!-- ante:footer=${encodeMetaValue(footer)} -->`;
  return `${headerComment}\n${footerComment}\n${html}`;
}

/** Get the saved snapshot for dirty comparison. */
export function getSavedSnapshot(): string {
  return savedSnapshot;
}

/** Reset the saved snapshot (e.g. for new file). */
export function resetSavedSnapshot(value: string = ''): void {
  savedSnapshot = value;
}

/** Check if an error is an AnteError with a specific kind. */
function isAnteError(err: unknown): err is AnteError {
  return (
    typeof err === 'object' &&
    err !== null &&
    'kind' in err &&
    'message' in err
  );
}

/** Show a user-facing error dialog. Silently ignores dialog_cancelled. */
async function showError(err: unknown): Promise<void> {
  if (isAnteError(err)) {
    if (err.kind === 'dialog_cancelled') return;
    const userMessage = ERROR_MESSAGES[err.kind] || 'An unexpected error occurred.';
    await message(userMessage, { title: 'ante', kind: 'error' });
  } else {
    await message('An unexpected error occurred.', { title: 'ante', kind: 'error' });
  }
}

/**
 * Callback type for getting/setting editor HTML content.
 * The page component provides these so file-ops stays editor-agnostic.
 */
export interface EditorBridge {
  getHTML: () => string;
  setHTML: (html: string) => void;
}

/**
 * Open a file via native dialog. Replaces the editor content on success.
 * Routes `.docx` through mammoth; everything else through the existing
 * HTML/text path.
 */
export async function openFile(bridge: EditorBridge): Promise<void> {
  try {
    const result = await invoke<OpenedFile>('open_file');
    await applyOpenedFile(result, bridge);
  } catch (err) {
    await showError(err);
  }
}

/**
 * Open a file at a known path (no native dialog). Used by the sidebar.
 * Prompts to save if the current document is dirty.
 */
export async function openPath(path: string, bridge: EditorBridge): Promise<void> {
  if (appState.isDirty) {
    const choice = await promptUnsavedChanges();
    if (choice === 'cancel') return;
    if (choice === 'save') {
      await saveFile(bridge);
      if (appState.isDirty) return;
    }
  }

  try {
    const result = await invoke<OpenedFile>('read_file', { path });
    await applyOpenedFile(result, bridge);
  } catch (err) {
    await showError(err);
  }
}

/**
 * Apply an OpenedFile payload to the editor, branching on kind.
 * - text: existing header/footer extraction + setHTML
 * - docx: mammoth-converted HTML, no header/footer (DOCX has its own model)
 */
async function applyOpenedFile(file: OpenedFile, bridge: EditorBridge): Promise<void> {
  if (file.kind === 'docx') {
    const { html, warnings } = await docxBytesToHtml(file.bytes_b64);
    appState.filePath = file.path;
    appState.headerText = '';
    appState.footerText = '';
    savedSnapshot = html;
    appState.isDirty = false;
    recentFiles.add(file.path);
    bridge.setHTML(html);
    if (warnings.length > 0) {
      // Surface the first 3 mammoth warnings so the user knows about lost
      // formatting (tables, footnotes, etc.) without a wall of text.
      const preview = warnings.slice(0, 3).join('\n');
      const more = warnings.length > 3 ? `\n... and ${warnings.length - 3} more` : '';
      await message(`Imported with some unsupported formatting:\n\n${preview}${more}`, {
        title: 'ante',
        kind: 'info',
      });
    }
    return;
  }

  const { header, footer, stripped } = parseHeaderFooter(file.contents);
  appState.filePath = file.path;
  appState.headerText = header;
  appState.footerText = footer;
  savedSnapshot = stripped;
  appState.isDirty = false;
  recentFiles.add(file.path);
  bridge.setHTML(stripped);
}

/**
 * Save the current document to its existing path.
 * If no path exists (untitled), falls through to saveFileAs.
 */
export async function saveFile(bridge: EditorBridge): Promise<void> {
  const editorHtml = bridge.getHTML();
  const contents = serializeWithHeaderFooter(
    editorHtml,
    appState.headerText,
    appState.footerText,
  );

  if (!appState.filePath) {
    await saveFileAs(bridge);
    return;
  }

  try {
    await invoke('save_file', { path: appState.filePath, contents });
    savedSnapshot = editorHtml;
    appState.isDirty = false;
  } catch (err) {
    await showError(err);
  }
}

/**
 * Save As: open a native save dialog and write to the chosen path.
 */
export async function saveFileAs(bridge: EditorBridge): Promise<void> {
  const editorHtml = bridge.getHTML();
  const contents = serializeWithHeaderFooter(
    editorHtml,
    appState.headerText,
    appState.footerText,
  );
  const suggestedName = appState.filePath
    ? appState.filePath.split(/[\\/]/).pop() || null
    : 'Untitled.html';

  try {
    const result = await invoke<SaveAsResult>('save_file_as', {
      contents,
      suggestedName,
    });
    appState.filePath = result.path;
    savedSnapshot = editorHtml;
    appState.isDirty = false;
    recentFiles.add(result.path);
  } catch (err) {
    await showError(err);
  }
}

/**
 * Export the current document to a `.docx` file via the native save dialog.
 * Header / footer text from app state is dropped; DOCX has its own model.
 */
export async function exportAsDocx(bridge: EditorBridge): Promise<void> {
  const editorHtml = bridge.getHTML();
  const suggestedName = appState.filePath
    ? appState.filePath.split(/[\\/]/).pop()?.replace(/\.[^./\\]+$/, '.docx') || 'Untitled.docx'
    : 'Untitled.docx';
  try {
    const bytesB64 = await htmlToDocxBytes(editorHtml);
    await invoke<SaveAsResult>('save_docx_as', {
      bytesB64,
      suggestedName,
    });
  } catch (err) {
    await showError(err);
  }
}

/**
 * Create a new empty file. Prompts to save if dirty.
 * Returns true if new file was created, false if cancelled.
 */
export async function newFile(bridge: EditorBridge): Promise<boolean> {
  if (appState.isDirty) {
    const shouldSave = await promptUnsavedChanges();
    if (shouldSave === 'cancel') return false;
    if (shouldSave === 'save') {
      await saveFile(bridge);
      // If still dirty after save attempt (e.g. save was cancelled), abort
      if (appState.isDirty) return false;
    }
  }

  appState.filePath = null;
  appState.headerText = '';
  appState.footerText = '';
  savedSnapshot = '';
  appState.isDirty = false;

  bridge.setHTML('');

  return true;
}

/**
 * Prompt user about unsaved changes.
 * Returns 'save', 'discard', or 'cancel'.
 *
 * Uses two sequential ask() dialogs to expose three outcomes, since
 * Tauri's ask() is binary. First dialog: Save vs continue without saving.
 * Second dialog (shown only when user declines save): Discard vs Cancel.
 */
export async function promptUnsavedChanges(): Promise<'save' | 'discard' | 'cancel'> {
  const { ask } = await import('@tauri-apps/plugin-dialog');

  const wantsSave = await ask(
    'You have unsaved changes. Save before continuing?',
    {
      title: 'ante',
      kind: 'warning',
      okLabel: 'Save',
      cancelLabel: "Don't Save",
    }
  );

  if (wantsSave) return 'save';

  // User chose not to save; confirm they really want to discard.
  const wantsDiscard = await ask(
    'Discard unsaved changes?',
    {
      title: 'ante',
      kind: 'warning',
      okLabel: 'Discard',
      cancelLabel: 'Cancel',
    }
  );

  return wantsDiscard ? 'discard' : 'cancel';
}
