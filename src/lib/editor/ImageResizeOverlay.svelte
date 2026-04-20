<script lang="ts">
  import { onMount } from 'svelte';
  import type { Editor } from '@tiptap/core';
  import { NodeSelection } from '@tiptap/pm/state';
  import { MIN_IMAGE_WIDTH } from './AnteImage';

  interface Props {
    editor: Editor | undefined;
  }

  let { editor }: Props = $props();

  type Rect = { left: number; top: number; width: number; height: number };
  type HandleDir = 'nw' | 'ne' | 'sw' | 'se';

  let rect = $state<Rect | null>(null);
  let previewRect = $state<Rect | null>(null);
  let selectedPos = $state<number | null>(null);
  let imageEl: HTMLImageElement | null = null;

  function measure(): void {
    if (!imageEl) {
      rect = null;
      return;
    }
    const r = imageEl.getBoundingClientRect();
    if (r.width === 0 && r.height === 0) {
      rect = null;
      return;
    }
    rect = { left: r.left, top: r.top, width: r.width, height: r.height };
  }

  function refresh(): void {
    if (!editor) {
      rect = null;
      imageEl = null;
      selectedPos = null;
      return;
    }
    const { selection } = editor.state;
    if (!(selection instanceof NodeSelection) || selection.node.type.name !== 'image') {
      rect = null;
      imageEl = null;
      selectedPos = null;
      return;
    }
    selectedPos = selection.from;
    const domNode = editor.view.nodeDOM(selection.from) as HTMLElement | null;
    const img =
      domNode instanceof HTMLImageElement
        ? domNode
        : (domNode?.querySelector('img') ?? null);
    imageEl = img;
    measure();
  }

  $effect(() => {
    if (!editor) return;
    refresh();
    const onUpdate = () => refresh();
    editor.on('selectionUpdate', onUpdate);
    editor.on('transaction', onUpdate);
    return () => {
      editor.off('selectionUpdate', onUpdate);
      editor.off('transaction', onUpdate);
    };
  });

  onMount(() => {
    const onScroll = () => measure();
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', onScroll);
    return () => {
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', onScroll);
    };
  });

  function startDrag(e: MouseEvent, dir: HandleDir): void {
    if (!editor || !rect || e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();

    const startX = e.clientX;
    const startY = e.clientY;
    const startRect = rect;
    const aspect = startRect.height > 0 ? startRect.width / startRect.height : 1;
    const growsWithDx = dir === 'ne' || dir === 'se';
    let moved = false;

    const onMove = (ev: MouseEvent): void => {
      const rawDx = ev.clientX - startX;
      if (!moved && Math.abs(rawDx) > 2) moved = true;
      if (!moved) return;
      const dx = growsWithDx ? rawDx : -rawDx;
      const w = Math.max(MIN_IMAGE_WIDTH, Math.round(startRect.width + dx));
      const h = Math.max(1, Math.round(w / aspect));
      const deltaW = w - startRect.width;
      const deltaH = h - startRect.height;
      const left = dir === 'nw' || dir === 'sw' ? startRect.left - deltaW : startRect.left;
      const top = dir === 'nw' || dir === 'ne' ? startRect.top - deltaH : startRect.top;
      previewRect = { left, top, width: w, height: h };
      void startY;
    };

    const onUp = (): void => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
      const committed = moved && previewRect;
      const commitWidth = committed ? previewRect!.width : null;
      previewRect = null;
      if (commitWidth !== null && selectedPos !== null && editor) {
        editor
          .chain()
          .setNodeSelection(selectedPos)
          .updateAttributes('image', { width: commitWidth })
          .run();
      }
    };

    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
  }

  const displayRect = $derived(previewRect ?? rect);
</script>

{#if displayRect}
  <div
    class="overlay"
    style="left:{displayRect.left}px;top:{displayRect.top}px;width:{displayRect.width}px;height:{displayRect.height}px;"
  >
    <span
      class="handle nw"
      role="button"
      tabindex="-1"
      aria-label="Resize from top-left"
      title="Resize from top-left (NW)"
      onmousedown={(e) => startDrag(e, 'nw')}
    ></span>
    <span
      class="handle ne"
      role="button"
      tabindex="-1"
      aria-label="Resize from top-right"
      title="Resize from top-right (NE)"
      onmousedown={(e) => startDrag(e, 'ne')}
    ></span>
    <span
      class="handle sw"
      role="button"
      tabindex="-1"
      aria-label="Resize from bottom-left"
      title="Resize from bottom-left (SW)"
      onmousedown={(e) => startDrag(e, 'sw')}
    ></span>
    <span
      class="handle se"
      role="button"
      tabindex="-1"
      aria-label="Resize from bottom-right"
      title="Resize from bottom-right (SE)"
      onmousedown={(e) => startDrag(e, 'se')}
    ></span>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    pointer-events: none;
    box-shadow: 0 0 0 2px var(--primary, oklch(0.205 0 0));
    border-radius: 2px;
    z-index: 50;
  }

  .handle {
    position: absolute;
    width: 12px;
    height: 12px;
    background: var(--primary, oklch(0.205 0 0));
    border: 2px solid var(--panel-bg, #ffffff);
    border-radius: 50%;
    box-shadow: 0 1px 3px color-mix(in srgb, #000 30%, transparent);
    pointer-events: auto;
    transform: translate(-50%, -50%);
    transition: transform 0.08s ease;
  }

  .handle:hover {
    transform: translate(-50%, -50%) scale(1.2);
  }

  .nw { top: 0; left: 0; cursor: nwse-resize; }
  .ne { top: 0; left: 100%; cursor: nesw-resize; }
  .sw { top: 100%; left: 0; cursor: nesw-resize; }
  .se { top: 100%; left: 100%; cursor: nwse-resize; }
</style>
