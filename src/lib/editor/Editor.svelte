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
  import { NodeSelection } from '@tiptap/pm/state';
  import { FontSize } from './FontSize';
  import { IndentExtension } from './IndentExtension';
  import { PaginationExtension } from './PaginationPlugin';
  import { GhostCompletion } from './GhostCompletion';
  import { AnteImage, ANTE_IMAGE_DRAG_EVENT, MIN_IMAGE_WIDTH } from './AnteImage';
  import ImageResizeOverlay from './ImageResizeOverlay.svelte';
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
  let imageMenuVisible = $state(false);
  let imageMenuX = $state(0);
  let imageMenuY = $state(0);

  function setImageLayout(layout: string): void {
    editor?.chain().focus().updateAttributes('image', { layout }).run();
  }

  function adjustImageWidth(factor: number): void {
    if (!editor) return;
    const { selection } = editor.state;
    if (!(selection instanceof NodeSelection) || selection.node.type.name !== 'image') return;
    const attrWidth = selection.node.attrs.width as number | null;
    let current: number | null = attrWidth;
    if (!current) {
      const domNode = editor.view.nodeDOM(selection.from) as HTMLElement | null;
      const imgEl = domNode?.querySelector('img');
      current = imgEl?.offsetWidth ?? null;
    }
    if (!current || !Number.isFinite(current)) return;
    const newWidth = Math.max(MIN_IMAGE_WIDTH, Math.round(current * factor));
    if (newWidth === current) return;
    editor.chain().focus().updateAttributes('image', { width: newWidth }).run();
    requestAnimationFrame(updateImageMenu);
  }

  function updateImageMenu(): void {
    if (!editor) { imageMenuVisible = false; return; }
    const { selection } = editor.state;
    if (!(selection instanceof NodeSelection) || selection.node.type.name !== 'image') {
      imageMenuVisible = false;
      return;
    }
    const domNode = editor.view.nodeDOM(selection.from) as HTMLElement | null;
    const img = domNode instanceof HTMLImageElement ? domNode : domNode?.querySelector('img');
    if (!img) { imageMenuVisible = false; return; }
    const rect = img.getBoundingClientRect();
    imageMenuX = rect.left + rect.width / 2;
    imageMenuY = rect.top - 8;
    imageMenuVisible = true;
  }

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
          types: ['heading', 'paragraph', 'image'],
        }),
        IndentExtension,
        GhostCompletion.configure({
          enabled: appState.aiEnabled,
        }),
        AnteImage,
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
        updateImageMenu();
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

    // Hide bubble menu while a float image is being dragged, restore after.
    const handleDragEvent = (e: Event) => {
      const phase = (e as CustomEvent<{ phase: 'start' | 'end' }>).detail?.phase;
      if (phase === 'start') {
        imageMenuVisible = false;
      } else if (phase === 'end') {
        requestAnimationFrame(updateImageMenu);
      }
    };
    editor.view.dom.addEventListener(ANTE_IMAGE_DRAG_EVENT, handleDragEvent);

    // Reposition the bubble menu when the user scrolls the document area.
    const scrollParent = container.closest('.document-area') as HTMLElement | null;
    let scrollRafId: number | undefined;
    const handleScroll = () => {
      if (!imageMenuVisible || scrollRafId != null) return;
      scrollRafId = requestAnimationFrame(() => {
        scrollRafId = undefined;
        updateImageMenu();
      });
    };
    scrollParent?.addEventListener('scroll', handleScroll, { passive: true });

    return () => {
      if (rafId != null) cancelAnimationFrame(rafId);
      if (scrollRafId != null) cancelAnimationFrame(scrollRafId);
      resizeObserver?.disconnect();
      editor?.view.dom.removeEventListener(ANTE_IMAGE_DRAG_EVENT, handleDragEvent);
      scrollParent?.removeEventListener('scroll', handleScroll);
      editor?.destroy();
      editor = undefined;
    };
  });
</script>

{#if imageMenuVisible}
  <div
    class="image-bubble-menu"
    role="toolbar"
    aria-label="Image layout"
    style="left:{imageMenuX}px;top:{imageMenuY}px;"
  >
    <button class="bubble-btn" title="Inline" aria-label="Set image layout to inline" onmousedown={(e) => { e.preventDefault(); setImageLayout('inline'); }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><line x1="3" y1="5" x2="21" y2="5"/><rect x="6" y="9" width="12" height="7" rx="1"/><line x1="3" y1="19" x2="21" y2="19"/></svg>
    </button>
    <button class="bubble-btn" title="Wrap left" aria-label="Wrap text around image on the left" onmousedown={(e) => { e.preventDefault(); setImageLayout('wrap-left'); }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="2" y="4" width="8" height="8" rx="1"/><line x1="13" y1="6" x2="22" y2="6"/><line x1="13" y1="10" x2="22" y2="10"/><line x1="2" y1="16" x2="22" y2="16"/><line x1="2" y1="20" x2="22" y2="20"/></svg>
    </button>
    <button class="bubble-btn" title="Wrap right" aria-label="Wrap text around image on the right" onmousedown={(e) => { e.preventDefault(); setImageLayout('wrap-right'); }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="14" y="4" width="8" height="8" rx="1"/><line x1="2" y1="6" x2="11" y2="6"/><line x1="2" y1="10" x2="11" y2="10"/><line x1="2" y1="16" x2="22" y2="16"/><line x1="2" y1="20" x2="22" y2="20"/></svg>
    </button>
    <button class="bubble-btn" title="Float (drag to reposition)" aria-label="Float image, drag to reposition" onmousedown={(e) => { e.preventDefault(); setImageLayout('float'); }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3v18"/><path d="M3 12h18"/><path d="M9 6l3-3 3 3"/><path d="M9 18l3 3 3-3"/><path d="M6 9l-3 3 3 3"/><path d="M18 9l3 3-3 3"/></svg>
    </button>
    <span class="bubble-divider" aria-hidden="true"></span>
    <button class="bubble-btn" title="Shrink image" aria-label="Shrink image" onmousedown={(e) => { e.preventDefault(); adjustImageWidth(0.9); }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="5" y1="12" x2="19" y2="12"/></svg>
    </button>
    <button class="bubble-btn" title="Enlarge image" aria-label="Enlarge image" onmousedown={(e) => { e.preventDefault(); adjustImageWidth(1.1); }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
    </button>
  </div>
{/if}
<div class="editor-container" bind:this={container}></div>
<ImageResizeOverlay {editor} />

<style>
  .image-bubble-menu {
    position: fixed;
    transform: translateX(-50%) translateY(-100%);
    display: flex;
    gap: 2px;
    background: var(--panel-bg, #fff);
    border: 1px solid var(--border-color, #e2e2e2);
    border-radius: 6px;
    padding: 3px;
    box-shadow: 0 2px 12px color-mix(in srgb, var(--fg, #000) 20%, transparent);
    z-index: 100;
    pointer-events: auto;
  }

  .bubble-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--fg, #111);
    cursor: pointer;
    opacity: 0.7;
    padding: 0;
    transition: background-color 0.1s, opacity 0.1s;
  }

  .bubble-btn:hover {
    background-color: var(--button-hover-bg, #f0f0f0);
    opacity: 1;
  }

  .bubble-divider {
    width: 1px;
    height: 18px;
    background-color: var(--border-color, #e2e2e2);
    margin: 0 2px;
    flex-shrink: 0;
  }

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

  /* Images */
  .editor-container :global(.tiptap img) {
    max-width: 100%;
    height: auto;
    border-radius: 2px;
  }

  .editor-container :global(.tiptap .ante-image-frame) {
    position: relative;
    display: inline-block;
    line-height: 0;
  }

  .editor-container :global(.tiptap .ante-image-caption) {
    display: block;
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 0.82em;
    font-style: italic;
    color: var(--gutter-fg, #666);
    text-align: center;
    margin-top: 6px;
    padding: 2px 4px;
    outline: none;
    min-height: 1em;
    line-height: 1.35;
    cursor: text;
    border-radius: 2px;
  }

  .editor-container :global(.tiptap .ante-image-caption:empty::before) {
    content: attr(data-placeholder);
    color: var(--gutter-fg, #aaa);
    opacity: 0.5;
  }

  .editor-container :global(.tiptap .ante-image-caption:focus) {
    background-color: color-mix(in srgb, var(--primary, #000) 6%, transparent);
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
