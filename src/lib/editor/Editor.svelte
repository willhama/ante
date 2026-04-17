<script lang="ts">
  import { onMount } from 'svelte';
  import { Editor } from '@tiptap/core';
  import StarterKit from '@tiptap/starter-kit';
  import Underline from '@tiptap/extension-underline';
  import Link from '@tiptap/extension-link';
  import TextAlign from '@tiptap/extension-text-align';
  import { TextStyle } from '@tiptap/extension-text-style';
  import Color from '@tiptap/extension-color';
  import FontFamily from '@tiptap/extension-font-family';
  import { FontSize } from './FontSize';
  import { IndentExtension } from './IndentExtension';
  import { PaginationExtension } from './PaginationPlugin';
  import { GhostCompletion } from './GhostCompletion';
  import { appState } from '$lib/state/app-state.svelte';

  interface Props {
    content?: string;
    onContentChange?: () => void;
    onSelectionUpdate?: () => void;
    onContentHeightChange?: (height: number) => void;
    onPageBreaksChange?: (pageBreaks: number[]) => void;
  }

  let { content = '', onContentChange, onSelectionUpdate, onContentHeightChange, onPageBreaksChange }: Props = $props();

  let container: HTMLDivElement;
  let editor = $state<Editor | undefined>(undefined);

  export function getEditor(): Editor | undefined {
    return editor;
  }

  export function getHTML(): string {
    return editor?.getHTML() ?? '';
  }

  export function setContent(html: string): void {
    editor?.commands.setContent(html, { emitUpdate: false });
  }

  export function setGhostEnabled(enabled: boolean): void {
    editor?.commands.setGhostEnabled(enabled);
  }

  onMount(() => {
    editor = new Editor({
      element: container,
      extensions: [
        StarterKit.configure({
          heading: {
            levels: [1, 2, 3],
          },
          // We register link + underline separately with custom config below.
          link: false,
          underline: false,
        }),
        TextStyle,
        FontFamily.configure({ types: ['textStyle'] }),
        Color.configure({ types: ['textStyle'] }),
        FontSize,
        Underline,
        Link.configure({
          openOnClick: false,
          HTMLAttributes: {
            rel: 'noopener noreferrer',
          },
        }),
        TextAlign.configure({
          types: ['heading', 'paragraph'],
        }),
        IndentExtension,
        GhostCompletion.configure({
          enabled: appState.aiEnabled,
        }),
        PaginationExtension.configure({
          contentHeightPerPage: appState.contentHeightPerPage,
          pageMargin: appState.pageMargin,
          pageGap: appState.pageGap,
          onPageBreaksChange: (breaks: number[]) => {
            onPageBreaksChange?.(breaks);
          },
        }),
      ],
      content,
      onUpdate: () => {
        onContentChange?.();
        onSelectionUpdate?.();
      },
      onSelectionUpdate: () => {
        onSelectionUpdate?.();
      },
      editorProps: {
        attributes: {
          class: 'ante-editor',
          spellcheck: 'true',
        },
      },
    });

    // Track editor content height via ResizeObserver for page count calculation.
    let resizeObserver: ResizeObserver | undefined;
    let rafId: number | undefined;
    const tiptapEl = container.querySelector('.tiptap');
    if (tiptapEl && onContentHeightChange) {
      resizeObserver = new ResizeObserver(() => {
        if (rafId != null) cancelAnimationFrame(rafId);
        rafId = requestAnimationFrame(() => {
          onContentHeightChange(tiptapEl.scrollHeight);
          rafId = undefined;
        });
      });
      resizeObserver.observe(tiptapEl);
    }

    return () => {
      if (rafId != null) cancelAnimationFrame(rafId);
      resizeObserver?.disconnect();
      editor?.destroy();
      editor = undefined;
    };
  });
</script>

<div class="editor-container" bind:this={container}></div>

<style>
  .editor-container {
    width: 100%;
    position: relative;
    z-index: 1;
  }

  .editor-container :global(.tiptap) {
    width: 100%;
    outline: none;
    min-height: 100%;
    font-family: Georgia, 'Times New Roman', serif;
    font-size: 17px;
    line-height: 1.6;
    color: var(--fg);
    -webkit-user-select: text;
    user-select: text;
    caret-color: var(--caret-color);
    /*
     * Padding is set inline by PageStack via CSS custom properties
     * so that editor content aligns with page rectangle margins.
     */
    padding: var(--editor-padding-top, 96px) var(--editor-padding-side, 96px);
  }

  /* Headings */
  .editor-container :global(.tiptap h1) {
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 32px;
    font-weight: 700;
    line-height: 1.2;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
    color: var(--fg);
  }

  .editor-container :global(.tiptap h2) {
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 26px;
    font-weight: 700;
    line-height: 1.3;
    margin-top: 1.4em;
    margin-bottom: 0.4em;
    color: var(--fg);
  }

  .editor-container :global(.tiptap h3) {
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 21px;
    font-weight: 700;
    line-height: 1.4;
    margin-top: 1.3em;
    margin-bottom: 0.3em;
    color: var(--fg);
  }

  /* Paragraphs */
  .editor-container :global(.tiptap p) {
    margin-bottom: 0.8em;
  }

  /* Lists */
  .editor-container :global(.tiptap ul),
  .editor-container :global(.tiptap ol) {
    padding-left: 24px;
    margin-bottom: 0.8em;
  }

  .editor-container :global(.tiptap li) {
    margin-bottom: 0.2em;
  }

  .editor-container :global(.tiptap li p) {
    margin-bottom: 0.2em;
  }

  /* Blockquote */
  .editor-container :global(.tiptap blockquote) {
    border-left: 3px solid var(--border-color);
    padding-left: 16px;
    font-style: italic;
    margin: 1em 0;
    color: var(--fg);
    opacity: 0.85;
  }

  /* Links */
  .editor-container :global(.tiptap a) {
    color: var(--accent-color, #4A90D9);
    text-decoration: underline;
    cursor: pointer;
  }

  /* Inline formatting */
  .editor-container :global(.tiptap strong) {
    font-weight: 700;
  }

  .editor-container :global(.tiptap em) {
    font-style: italic;
  }

  .editor-container :global(.tiptap u) {
    text-decoration: underline;
  }

  .editor-container :global(.tiptap s) {
    text-decoration: line-through;
  }

  /* Code */
  .editor-container :global(.tiptap code) {
    background-color: var(--button-bg);
    border-radius: 3px;
    padding: 0.15em 0.3em;
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 0.9em;
  }

  /* Horizontal rule */
  .editor-container :global(.tiptap hr) {
    border: none;
    border-top: 1px solid var(--border-color);
    margin: 1.5em 0;
  }

  /* First child: no top margin */
  .editor-container :global(.tiptap > *:first-child) {
    margin-top: 0;
  }

  /* Placeholder for empty editor */
  .editor-container :global(.tiptap p.is-editor-empty:first-child::before) {
    content: 'Start writing...';
    color: var(--gutter-fg);
    font-style: italic;
    pointer-events: none;
    float: left;
    height: 0;
  }
</style>
