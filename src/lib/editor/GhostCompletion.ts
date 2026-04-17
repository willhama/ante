import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { EditorState, Transaction } from '@tiptap/pm/state';
import type { EditorView } from '@tiptap/pm/view';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    ghostCompletion: {
      setGhostEnabled: (enabled: boolean) => ReturnType;
    };
  }
}

export interface GhostCompletionOptions {
  enabled: boolean;
  debounceMs: number;
  contextBeforeChars: number;
  contextAfterChars: number;
  minContextChars: number;
  cooldownMs: number;
}

interface GhostState {
  enabled: boolean;
  suggestion: string;
  anchor: number | null;
  requestId: string | null;
}

interface GhostMeta {
  kind:
    | 'set-enabled'
    | 'start-request'
    | 'append-chunk'
    | 'clear'
    | 'accept';
  enabled?: boolean;
  requestId?: string;
  anchor?: number;
  text?: string;
}

interface ChunkPayload {
  id: string;
  text: string;
}

interface DonePayload {
  id: string;
}

interface ErrorPayload {
  id: string;
  message: string;
}

export const ghostPluginKey = new PluginKey<GhostState>('ghost-completion');
const META_KEY = 'ghost-completion-meta';

/**
 * Extracts plain-text context before and after the cursor.
 * Walks text nodes, skipping non-text. Formatting is stripped since this is
 * only used as prompt context.
 */
function extractContext(
  state: EditorState,
  beforeChars: number,
  afterChars: number,
): { before: string; after: string } {
  const pos = state.selection.from;
  const before = state.doc.textBetween(Math.max(0, pos - beforeChars), pos, '\n', ' ');
  const after = state.doc.textBetween(
    pos,
    Math.min(state.doc.content.size, pos + afterChars),
    '\n',
    ' ',
  );
  return { before, after };
}

function createGhostWidget(text: string): HTMLElement {
  const span = document.createElement('span');
  span.className = 'ghost-text';
  span.textContent = text;
  span.setAttribute('contenteditable', 'false');
  return span;
}

/**
 * Build decorations against the current doc. The DecorationSet.create needs
 * a Node as its first argument; callers pass the current doc.
 */
function buildDecorationsForDoc(
  doc: EditorState['doc'],
  suggestion: string,
  anchor: number | null,
): DecorationSet {
  if (!suggestion || anchor === null) return DecorationSet.empty;
  const captured = suggestion;
  const widget = Decoration.widget(anchor, () => createGhostWidget(captured), {
    side: 1,
    key: `ghost-${anchor}-${suggestion.length}-${suggestion.slice(-16)}`,
  });
  return DecorationSet.create(doc, [widget]);
}

export const GhostCompletion = Extension.create<GhostCompletionOptions>({
  name: 'ghostCompletion',

  // Higher priority than IndentExtension (default 100) so Tab is captured
  // for accepting a ghost suggestion before indent fires.
  priority: 200,

  addOptions() {
    return {
      enabled: true,
      debounceMs: 800,
      contextBeforeChars: 500,
      contextAfterChars: 200,
      minContextChars: 10,
      cooldownMs: 2000,
    };
  },

  addCommands() {
    return {
      setGhostEnabled:
        (enabled: boolean) =>
        ({ tr, dispatch }) => {
          if (dispatch) {
            const meta: GhostMeta = { kind: 'set-enabled', enabled };
            tr.setMeta(META_KEY, meta);
            dispatch(tr);
          }
          return true;
        },
    };
  },

  addKeyboardShortcuts() {
    return {
      Tab: ({ editor }) => {
        const state = ghostPluginKey.getState(editor.state);
        if (!state || !state.suggestion || state.anchor === null) {
          return false;
        }
        const { suggestion, anchor } = state;
        if (state.requestId) {
          invoke('cancel_completion', { requestId: state.requestId }).catch(() => {});
        }
        const tr = editor.state.tr;
        tr.insertText(suggestion, anchor);
        const meta: GhostMeta = { kind: 'accept' };
        tr.setMeta(META_KEY, meta);
        editor.view.dispatch(tr);
        return true;
      },
      Escape: ({ editor }) => {
        const state = ghostPluginKey.getState(editor.state);
        if (!state || !state.suggestion) return false;
        const tr = editor.state.tr;
        const meta: GhostMeta = { kind: 'clear' };
        tr.setMeta(META_KEY, meta);
        editor.view.dispatch(tr);
        // Best-effort cancel the in-flight stream.
        if (state.requestId) {
          invoke('cancel_completion', { requestId: state.requestId }).catch(() => {});
        }
        return true;
      },
    };
  },

  addProseMirrorPlugins() {
    const options = this.options;

    return [
      new Plugin<GhostState>({
        key: ghostPluginKey,

        state: {
          init(): GhostState {
            return {
              enabled: options.enabled,
              suggestion: '',
              anchor: null,
              requestId: null,
            };
          },
          apply(tr: Transaction, value: GhostState, oldState, newState): GhostState {
            const meta = tr.getMeta(META_KEY) as GhostMeta | undefined;

            if (meta) {
              switch (meta.kind) {
                case 'set-enabled':
                  return {
                    ...value,
                    enabled: meta.enabled ?? value.enabled,
                    suggestion: '',
                    anchor: null,
                    requestId: null,
                  };
                case 'start-request':
                  return {
                    ...value,
                    suggestion: '',
                    anchor: meta.anchor ?? value.anchor,
                    requestId: meta.requestId ?? null,
                  };
                case 'append-chunk':
                  // Ignore chunks for stale requests
                  if (!meta.requestId || meta.requestId !== value.requestId) {
                    return value;
                  }
                  return {
                    ...value,
                    suggestion: value.suggestion + (meta.text ?? ''),
                  };
                case 'accept':
                case 'clear':
                  return {
                    ...value,
                    suggestion: '',
                    anchor: null,
                    requestId: null,
                  };
              }
            }

            // Any doc change or selection change not caused by our meta clears the ghost.
            const selectionChanged =
              !oldState.selection.eq(newState.selection);
            if (tr.docChanged || selectionChanged) {
              if (value.suggestion || value.anchor !== null || value.requestId) {
                return {
                  ...value,
                  suggestion: '',
                  anchor: null,
                  requestId: null,
                };
              }
            }
            return value;
          },
        },

        props: {
          decorations(state: EditorState): DecorationSet | null {
            const gs = ghostPluginKey.getState(state);
            if (!gs) return null;
            return buildDecorationsForDoc(state.doc, gs.suggestion, gs.anchor);
          },
        },

        view(view: EditorView) {
          let debounceTimer: number | null = null;
          let unlistenChunk: UnlistenFn | null = null;
          let unlistenDone: UnlistenFn | null = null;
          let unlistenError: UnlistenFn | null = null;
          let destroyed = false;
          let currentGeneration = 0;
          let lastRequestStartedAt = 0;
          let rateLimitedUntil = 0;

          function clearDebounce(): void {
            if (debounceTimer !== null) {
              window.clearTimeout(debounceTimer);
              debounceTimer = null;
            }
          }

          function cancelInFlight(state: EditorState): void {
            const gs = ghostPluginKey.getState(state);
            if (gs?.requestId) {
              const id = gs.requestId;
              invoke('cancel_completion', { requestId: id }).catch(() => {});
            }
          }

          function scheduleRequest(): void {
            clearDebounce();
            const gs = ghostPluginKey.getState(view.state);
            if (!gs?.enabled) return;
            // Don't stack requests if a suggestion is already visible; user
            // should accept (Tab) or dismiss (Esc) first.
            if (gs.suggestion || gs.requestId) return;
            // Only trigger when selection is a cursor (empty).
            const sel = view.state.selection;
            if (sel.from !== sel.to) return;

            const now = Date.now();
            // Cooldown: minimum interval between successive request starts.
            if (now - lastRequestStartedAt < options.cooldownMs) return;
            // Rate-limit backoff: if we were 429'd, wait.
            if (now < rateLimitedUntil) return;

            const { before } = extractContext(
              view.state,
              options.contextBeforeChars,
              options.contextAfterChars,
            );
            if (before.length < options.minContextChars) return;

            debounceTimer = window.setTimeout(() => {
              debounceTimer = null;
              if (destroyed) return;
              const current = ghostPluginKey.getState(view.state);
              if (!current?.enabled) return;
              if (current.suggestion || current.requestId) return;
              const sel2 = view.state.selection;
              if (sel2.from !== sel2.to) return;
              const anchor = sel2.from;
              const { before: ctxBefore, after: ctxAfter } = extractContext(
                view.state,
                options.contextBeforeChars,
                options.contextAfterChars,
              );
              if (ctxBefore.length < options.minContextChars) return;

              const gen = ++currentGeneration;
              lastRequestStartedAt = Date.now();

              invoke<string>('stream_completion', {
                contextBefore: ctxBefore,
                contextAfter: ctxAfter,
              })
                .then((requestId) => {
                  if (destroyed || view.isDestroyed) return;
                  if (gen !== currentGeneration) {
                    invoke('cancel_completion', { requestId }).catch(() => {});
                    return;
                  }
                  const tr = view.state.tr;
                  const meta: GhostMeta = {
                    kind: 'start-request',
                    requestId,
                    anchor,
                  };
                  tr.setMeta(META_KEY, meta);
                  view.dispatch(tr);
                })
                .catch(() => {
                  // Silent. Includes the "not configured" case.
                });
            }, options.debounceMs);
          }

          // Wire up Tauri event listeners.
          (async () => {
            try {
              const chunkFn = await listen<ChunkPayload>('completion-chunk', (evt) => {
                if (destroyed || view.isDestroyed) return;
                const gs = ghostPluginKey.getState(view.state);
                if (!gs || gs.requestId !== evt.payload.id) return;
                const tr = view.state.tr;
                const meta: GhostMeta = {
                  kind: 'append-chunk',
                  requestId: evt.payload.id,
                  text: evt.payload.text,
                };
                tr.setMeta(META_KEY, meta);
                view.dispatch(tr);
              });
              if (destroyed) {
                chunkFn();
                return;
              }
              unlistenChunk = chunkFn;

              const doneFn = await listen<DonePayload>('completion-done', (_evt) => {
                // Keep current suggestion visible; nothing to do.
              });
              if (destroyed) {
                doneFn();
                unlistenChunk?.();
                unlistenChunk = null;
                return;
              }
              unlistenDone = doneFn;

              const errorFn = await listen<ErrorPayload>('completion-error', (evt) => {
                if (destroyed || view.isDestroyed) return;
                if (evt.payload.message === 'rate limited') {
                  // Back off for 30s on 429 responses.
                  rateLimitedUntil = Date.now() + 30_000;
                }
                const gs = ghostPluginKey.getState(view.state);
                if (!gs || gs.requestId !== evt.payload.id) return;
                const tr = view.state.tr;
                const meta: GhostMeta = { kind: 'clear' };
                tr.setMeta(META_KEY, meta);
                view.dispatch(tr);
              });
              if (destroyed) {
                errorFn();
                unlistenDone?.();
                unlistenChunk?.();
                unlistenDone = null;
                unlistenChunk = null;
                return;
              }
              unlistenError = errorFn;
            } catch {
              // Not in a Tauri environment (e.g., vite dev in a plain browser).
            }
          })();

          return {
            update(newView, prevState) {
              const newState = newView.state;
              const docChanged = !newState.doc.eq(prevState.doc);
              const selChanged = !newState.selection.eq(prevState.selection);
              if (!docChanged && !selChanged) return;

              // Read the requestId that was active BEFORE the transaction.
              // The apply reducer clears requestId on doc/selection changes, so
              // reading from newState would miss the id we need to cancel.
              const prev = ghostPluginKey.getState(prevState);
              const curr = ghostPluginKey.getState(newState);
              if (prev?.requestId && prev.requestId !== curr?.requestId) {
                invoke('cancel_completion', { requestId: prev.requestId }).catch(() => {});
              }

              if (!curr?.enabled) {
                clearDebounce();
                return;
              }

              if (docChanged) {
                scheduleRequest();
              }
            },
            destroy() {
              destroyed = true;
              clearDebounce();
              cancelInFlight(view.state);
              unlistenChunk?.();
              unlistenDone?.();
              unlistenError?.();
            },
          };
        },
      }),
    ];
  },
});
