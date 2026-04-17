import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { EditorView } from '@tiptap/pm/view';
import type { EditorState } from '@tiptap/pm/state';

export interface PaginationOptions {
  contentHeightPerPage: number;
  pageMargin: number;
  pageGap: number;
  onPageBreaksChange?: (pageBreaks: number[]) => void;
}

const paginationPluginKey = new PluginKey('pagination');

function createSpacerDOM(height: number): HTMLElement {
  const el = document.createElement('div');
  el.className = 'page-spacer';
  el.style.height = `${height}px`;
  el.style.pointerEvents = 'none';
  el.style.userSelect = 'none';
  el.setAttribute('contenteditable', 'false');
  return el;
}

function computePageBreaks(
  view: EditorView,
  options: PaginationOptions,
): { decorations: DecorationSet; pageBreaks: number[] } {
  const { contentHeightPerPage, pageMargin, pageGap } = options;
  const spacerHeight = pageMargin * 2 + pageGap;
  const doc = view.state.doc;
  const decorations: Decoration[] = [];
  const pageBreaks: number[] = [];

  // Accumulated content height on the current page
  let usedOnPage = 0;

  // Walk top-level nodes
  for (let i = 0; i < doc.childCount; i++) {
    const child = doc.child(i);
    const pos = posOfChild(doc, i);

    // Get the DOM element for this node
    const domNode = view.nodeDOM(pos);
    if (!domNode || !(domNode instanceof HTMLElement)) continue;

    const rect = domNode.getBoundingClientRect();
    const nodeHeight = rect.height;

    // Skip the very first node (never insert a spacer before it)
    if (i === 0) {
      usedOnPage = nodeHeight;
      continue;
    }

    // Would this node overflow the current page?
    if (usedOnPage + nodeHeight > contentHeightPerPage && usedOnPage > 0) {
      // Compute how much remaining space on this page plus the gap/margins
      const remainingOnPage = contentHeightPerPage - usedOnPage;
      const actualSpacerHeight = remainingOnPage + spacerHeight;

      // Insert spacer decoration before this node
      const capturedHeight = actualSpacerHeight;
      const widget = Decoration.widget(pos, () => createSpacerDOM(capturedHeight), {
        side: -1,
        key: `page-spacer-${pos}`,
      });
      decorations.push(widget);

      // Record the page break (pixel offset from editor top where the break occurs)
      pageBreaks.push(pageBreaks.length + 1);

      // This node starts fresh on the new page
      usedOnPage = nodeHeight;
    } else {
      usedOnPage += nodeHeight;
    }

    // If a single node is taller than a page, just let it overflow
    // (no spacer before it, already handled above by checking usedOnPage > 0)
  }

  return {
    decorations: DecorationSet.create(doc, decorations),
    pageBreaks,
  };
}

/** Get the position of the i-th top-level child in the document. */
function posOfChild(doc: EditorState['doc'], index: number): number {
  let pos = 0;
  for (let i = 0; i < index; i++) {
    pos += doc.child(i).nodeSize;
  }
  return pos;
}

function createPaginationPlugin(options: PaginationOptions): Plugin {
  let currentOptions = { ...options };
  let rafScheduled = false;

  return new Plugin({
    key: paginationPluginKey,

    state: {
      init(): DecorationSet {
        return DecorationSet.empty;
      },
      apply(_tr, oldSet): DecorationSet {
        // Actual recalculation happens in the view via updateDecorations
        return oldSet;
      },
    },

    props: {
      decorations(state: EditorState): DecorationSet {
        return paginationPluginKey.getState(state) ?? DecorationSet.empty;
      },
    },

    view(view: EditorView) {
      function updateDecorations(): void {
        const { decorations, pageBreaks } = computePageBreaks(view, currentOptions);

        // Only update if decorations actually changed
        const oldDecos = paginationPluginKey.getState(view.state);
        if (oldDecos === decorations) return;

        // Dispatch a metadata-only transaction to update the plugin state
        const tr = view.state.tr.setMeta(paginationPluginKey, { decorations });
        view.dispatch(tr);

        currentOptions.onPageBreaksChange?.(pageBreaks);
      }

      function scheduleUpdate(): void {
        if (rafScheduled) return;
        rafScheduled = true;
        requestAnimationFrame(() => {
          rafScheduled = false;
          if (view.isDestroyed) return;
          updateDecorations();
        });
      }

      // Initial calculation after the view is ready
      scheduleUpdate();

      return {
        update(): void {
          scheduleUpdate();
        },
        destroy(): void {
          rafScheduled = false;
        },
      };
    },
  });
}

export const PaginationExtension = Extension.create<PaginationOptions>({
  name: 'pagination',

  addOptions() {
    return {
      contentHeightPerPage: 864,
      pageMargin: 96,
      pageGap: 24,
      onPageBreaksChange: undefined,
    };
  },

  addProseMirrorPlugins() {
    const plugin = createPaginationPlugin(this.options);

    // Override the plugin state apply to handle our meta transactions
    const originalApply = plugin.spec.state!.apply!;
    plugin.spec.state!.apply = function (tr, oldValue, _oldState, _newState) {
      const meta = tr.getMeta(paginationPluginKey);
      if (meta?.decorations) {
        return meta.decorations;
      }
      if (tr.docChanged) {
        // Map decorations through document changes
        return (oldValue as DecorationSet).map(tr.mapping, tr.doc);
      }
      return oldValue;
    };

    return [plugin];
  },
});
