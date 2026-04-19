<script lang="ts">
  import { appState } from '$lib/state/app-state.svelte';
  import type { Snippet } from 'svelte';

  interface Props {
    contentHeight: number;
    pageBreaks?: number[];
    children: Snippet;
  }

  let { contentHeight, pageBreaks = [], children }: Props = $props();

  /**
   * Number of visual page backgrounds to render.
   *
   * Prefer the pagination plugin's break count (content-aware via node
   * measurement + spacer decorations). Fall back to naive height math
   * if the plugin hasn't reported yet. The plugin-driven count matches
   * exactly where spacer decorations push content to the next page zone.
   */
  let pageCount = $derived.by(() => {
    if (pageBreaks.length > 0) {
      return pageBreaks.length + 1;
    }
    return Math.max(1, Math.ceil(contentHeight / appState.contentHeightPerPage));
  });

  /** Total height of all page backgrounds + gaps between them. */
  let totalStackHeight = $derived(
    pageCount * appState.pageHeight + (pageCount - 1) * appState.pageGap
  );

  /** Minimum height so the editor always fills at least one full page. */
  let minEditorHeight = $derived(appState.pageHeight);

  /**
   * Only the first page's header/footer is directly editable. Other pages
   * render read-only mirrors that sync through appState. This avoids the
   * focus-loss that would occur if Svelte overwrote a contenteditable div's
   * text content while the user was typing into it.
   */
  let headerEditable: HTMLDivElement | undefined = $state();
  let footerEditable: HTMLDivElement | undefined = $state();

  /**
   * Keep the editable divs' innerText in sync with state when state changes
   * from an outside source (e.g. after opening a file). We only overwrite
   * when the DOM text actually differs to avoid clobbering a typing caret.
   */
  $effect(() => {
    const value = appState.headerText;
    if (headerEditable && headerEditable.innerText !== value) {
      headerEditable.innerText = value;
    }
  });

  $effect(() => {
    const value = appState.footerText;
    if (footerEditable && footerEditable.innerText !== value) {
      footerEditable.innerText = value;
    }
  });

  function handleHeaderInput(e: Event): void {
    const el = e.currentTarget as HTMLDivElement;
    appState.headerText = el.innerText;
    appState.isDirty = true;
  }

  function handleFooterInput(e: Event): void {
    const el = e.currentTarget as HTMLDivElement;
    appState.footerText = el.innerText;
    appState.isDirty = true;
  }

  /** Focus the page-1 editable when the user clicks a page-N (N>1) zone. */
  function focusHeaderEditor(): void {
    headerEditable?.focus();
  }

  function focusFooterEditor(): void {
    footerEditable?.focus();
  }
</script>

<div
  class="document-area"
  style:--page-width="{appState.pageWidth}px"
  style:--page-height="{appState.pageHeight}px"
  style:--page-margin="{appState.pageMargin}px"
  style:--page-gap="{appState.pageGap}px"
>
  <div
    class="page-stack"
    style:width="{appState.pageWidth}px"
    style:min-height="{totalStackHeight}px"
  >
    <!-- Page background rectangles (purely decorative) -->
    {#each { length: pageCount } as _, i}
      <div
        class="page-bg"
        style:top="{i * (appState.pageHeight + appState.pageGap)}px"
        style:height="{appState.pageHeight}px"
        style:width="{appState.pageWidth}px"
      ></div>

      {#if i === 0}
        <div
          bind:this={headerEditable}
          class="page-header editable"
          contenteditable="plaintext-only"
          spellcheck="false"
          data-placeholder="Header"
          role="textbox"
          tabindex="0"
          aria-label="Page header"
          style:top="{i * (appState.pageHeight + appState.pageGap) + 24}px"
          style:left="{appState.pageMargin}px"
          style:width="{appState.pageWidth - 2 * appState.pageMargin}px"
          oninput={handleHeaderInput}
        ></div>
      {:else}
        <div
          class="page-header mirror"
          style:top="{i * (appState.pageHeight + appState.pageGap) + 24}px"
          style:left="{appState.pageMargin}px"
          style:width="{appState.pageWidth - 2 * appState.pageMargin}px"
          onclick={focusHeaderEditor}
          onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && focusHeaderEditor()}
          role="button"
          tabindex="0"
          aria-label="Edit page header (focuses page 1)"
        >{appState.headerText}</div>
      {/if}

      {#if i === 0}
        <div
          bind:this={footerEditable}
          class="page-footer editable"
          contenteditable="plaintext-only"
          spellcheck="false"
          data-placeholder="Footer"
          role="textbox"
          tabindex="0"
          aria-label="Page footer"
          style:top="{i * (appState.pageHeight + appState.pageGap) + appState.pageHeight - 48 - 32}px"
          style:left="{appState.pageMargin}px"
          style:width="{appState.pageWidth - 2 * appState.pageMargin}px"
          oninput={handleFooterInput}
        ></div>
      {:else}
        <div
          class="page-footer mirror"
          style:top="{i * (appState.pageHeight + appState.pageGap) + appState.pageHeight - 48 - 32}px"
          style:left="{appState.pageMargin}px"
          style:width="{appState.pageWidth - 2 * appState.pageMargin}px"
          onclick={focusFooterEditor}
          onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && focusFooterEditor()}
          role="button"
          tabindex="0"
          aria-label="Edit page footer (focuses page 1)"
        >{appState.footerText}</div>
      {/if}

      <div
        class="page-number"
        style:top="{i * (appState.pageHeight + appState.pageGap) + appState.pageHeight - 16}px"
      >
        {i + 1}
      </div>
    {/each}

    <!-- Editor overlay: positioned relative, padding aligns content with page margins -->
    <div
      class="editor-wrapper"
      style:--editor-padding-top="{appState.pageMargin}px"
      style:--editor-padding-side="{appState.pageMargin}px"
      style:min-height="{minEditorHeight}px"
    >
      {@render children()}
    </div>
  </div>
</div>

<style>
  .document-area {
    flex: 1;
    overflow-y: auto;
    overflow-x: auto;
    background-color: var(--document-bg);
    display: flex;
    justify-content: center;
    padding: 32px 0;
    scrollbar-width: none;
  }

  .document-area::-webkit-scrollbar {
    display: none;
  }

  .page-stack {
    position: relative;
    flex-shrink: 0;
  }

  .page-bg {
    position: absolute;
    left: 0;
    background-color: var(--page-bg);
    box-shadow: var(--page-shadow);
    border-radius: 2px;
    pointer-events: none;
  }

  .page-number {
    position: absolute;
    right: 16px;
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 11px;
    color: var(--gutter-fg);
    pointer-events: none;
    user-select: none;
    text-align: right;
  }

  .editor-wrapper {
    position: relative;
    z-index: 1;
    width: 100%;
  }
</style>
