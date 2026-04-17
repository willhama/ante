import { Extension } from '@tiptap/core';
import type { Editor } from '@tiptap/core';

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    indent: {
      indent: () => ReturnType;
      outdent: () => ReturnType;
    };
  }
}

const INDENT_ATTR = 'indent';
const MAX_INDENT = 8;
const INDENT_PX = 32;
const INDENTABLE_NODES = ['paragraph', 'heading'];

function isInList(editor: Editor): boolean {
  const { $from } = editor.state.selection;
  for (let depth = $from.depth; depth > 0; depth--) {
    if ($from.node(depth).type.name === 'listItem') return true;
  }
  return false;
}

export const IndentExtension = Extension.create({
  name: 'indent',

  addGlobalAttributes() {
    return [
      {
        types: INDENTABLE_NODES,
        attributes: {
          [INDENT_ATTR]: {
            default: 0,
            parseHTML: (element) => {
              const raw = element.style.marginLeft;
              if (!raw) return 0;
              const px = parseInt(raw, 10);
              if (isNaN(px) || px <= 0) return 0;
              return Math.min(Math.round(px / INDENT_PX), MAX_INDENT);
            },
            renderHTML: (attributes) => {
              const level = attributes[INDENT_ATTR] as number;
              if (!level || level <= 0) return {};
              return { style: `margin-left: ${level * INDENT_PX}px` };
            },
          },
        },
      },
    ];
  },

  addCommands() {
    return {
      indent:
        () =>
        ({ tr, state, dispatch }) => {
          const { from, to } = state.selection;
          let changed = false;

          state.doc.nodesBetween(from, to, (node, pos) => {
            if (!INDENTABLE_NODES.includes(node.type.name)) return;
            const current = (node.attrs[INDENT_ATTR] as number) ?? 0;
            if (current >= MAX_INDENT) return;
            tr.setNodeMarkup(pos, undefined, {
              ...node.attrs,
              [INDENT_ATTR]: Math.min(current + 1, MAX_INDENT),
            });
            changed = true;
          });

          if (changed && dispatch) dispatch(tr);
          return changed;
        },

      outdent:
        () =>
        ({ tr, state, dispatch }) => {
          const { from, to } = state.selection;
          let changed = false;

          state.doc.nodesBetween(from, to, (node, pos) => {
            if (!INDENTABLE_NODES.includes(node.type.name)) return;
            const current = (node.attrs[INDENT_ATTR] as number) ?? 0;
            if (current <= 0) return;
            tr.setNodeMarkup(pos, undefined, {
              ...node.attrs,
              [INDENT_ATTR]: Math.max(current - 1, 0),
            });
            changed = true;
          });

          if (changed && dispatch) dispatch(tr);
          return changed;
        },
    };
  },

  addKeyboardShortcuts() {
    return {
      Tab: ({ editor }) => {
        if (isInList(editor)) {
          return editor.commands.sinkListItem('listItem');
        }
        return editor.commands.indent();
      },

      'Shift-Tab': ({ editor }) => {
        if (isInList(editor)) {
          return editor.commands.liftListItem('listItem');
        }
        return editor.commands.outdent();
      },
    };
  },
});
