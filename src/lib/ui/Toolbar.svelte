<script lang="ts">
  import type { Editor } from '@tiptap/core';
  import type { PageSize } from '$lib/types';
  import { appState } from '$lib/state/app-state.svelte';
  import AiStatusDot from '$lib/ui/AiStatusDot.svelte';

  interface Props {
    editor: Editor | undefined;
    onNew?: () => void;
    onOpen?: () => void;
    onSave?: () => void;
    onToggleAi?: () => void;
  }

  let { editor, onNew, onOpen, onSave, onToggleAi }: Props = $props();

  function setPageSize(e: Event): void {
    appState.pageSize = (e.target as HTMLSelectElement).value as PageSize;
  }

  /**
   * Tick counter incremented on editor updates to trigger
   * reactive reads of editor.isActive() in Svelte 5.
   */
  let tick = $state(0);

  export function bumpTick(): void {
    tick++;
  }

  function isActive(name: string, attrs?: Record<string, unknown>): boolean {
    void tick;
    if (!editor) return false;
    return attrs ? editor.isActive(name, attrs) : editor.isActive(name);
  }

  function currentBlockType(): string {
    void tick;
    if (!editor) return 'paragraph';
    if (editor.isActive('heading', { level: 1 })) return 'h1';
    if (editor.isActive('heading', { level: 2 })) return 'h2';
    if (editor.isActive('heading', { level: 3 })) return 'h3';
    if (editor.isActive('blockquote')) return 'blockquote';
    return 'paragraph';
  }

  function setBlockType(e: Event): void {
    if (!editor) return;
    const value = (e.target as HTMLSelectElement).value;
    const chain = editor.chain().focus();
    switch (value) {
      case 'paragraph':
        chain.setParagraph().run();
        break;
      case 'h1':
        chain.toggleHeading({ level: 1 }).run();
        break;
      case 'h2':
        chain.toggleHeading({ level: 2 }).run();
        break;
      case 'h3':
        chain.toggleHeading({ level: 3 }).run();
        break;
      case 'blockquote':
        chain.toggleBlockquote().run();
        break;
    }
  }

  const FONT_FAMILIES = [
    'Georgia',
    'Helvetica',
    'Arial',
    'Times New Roman',
    'Verdana',
    'Courier New',
    'Cambria',
    'Calibri',
  ];

  const FONT_SIZES = [10, 12, 14, 16, 17, 18, 20, 24, 28, 32, 36, 48, 64];
  const DEFAULT_FONT_FAMILY = 'Georgia';
  const DEFAULT_FONT_SIZE = 17;

  function currentFontFamily(): string {
    void tick;
    if (!editor) return DEFAULT_FONT_FAMILY;
    return editor.getAttributes('textStyle').fontFamily ?? DEFAULT_FONT_FAMILY;
  }

  function currentFontSize(): number {
    void tick;
    if (!editor) return DEFAULT_FONT_SIZE;
    const raw: string | undefined = editor.getAttributes('textStyle').fontSize;
    if (!raw) return DEFAULT_FONT_SIZE;
    const parsed = parseInt(raw, 10);
    return isNaN(parsed) ? DEFAULT_FONT_SIZE : parsed;
  }

  function setFontFamily(e: Event): void {
    if (!editor) return;
    const value = (e.target as HTMLSelectElement).value;
    if (value === DEFAULT_FONT_FAMILY) {
      editor.chain().focus().unsetFontFamily().run();
    } else {
      editor.chain().focus().setFontFamily(value).run();
    }
  }

  function setFontSize(e: Event): void {
    if (!editor) return;
    const value = (e.target as HTMLSelectElement).value;
    editor.chain().focus().setFontSize(`${value}px`).run();
  }

  function setColor(e: Event): void {
    if (!editor) return;
    const value = (e.target as HTMLInputElement).value;
    editor.chain().focus().setColor(value).run();
  }

  function currentColor(): string {
    void tick;
    if (!editor) return '#000000';
    return editor.getAttributes('textStyle').color ?? '#000000';
  }

  function insertLink(): void {
    if (!editor) return;
    const previousUrl = editor.getAttributes('link').href ?? '';
    const url = window.prompt('Enter URL:', previousUrl);
    if (url === null) return;
    if (url === '') {
      editor.chain().focus().extendMarkRange('link').unsetLink().run();
      return;
    }
    editor.chain().focus().extendMarkRange('link').setLink({ href: url }).run();
  }

  function openSettings(): void {
    appState.settingsOpen = true;
  }
</script>

<div class="toolbar" role="toolbar" aria-label="Formatting toolbar">
  <!-- File group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      title="New (Cmd+N)"
      onclick={onNew}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
        <polyline points="14 2 14 8 20 8"></polyline>
        <line x1="12" y1="18" x2="12" y2="12"></line>
        <line x1="9" y1="15" x2="15" y2="15"></line>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      title="Open (Cmd+O)"
      onclick={onOpen}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      title="Save (Cmd+S)"
      onclick={onSave}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path>
        <polyline points="17 21 17 13 7 13 7 21"></polyline>
        <polyline points="7 3 7 8 15 8"></polyline>
      </svg>
    </button>
  </div>

  <div class="toolbar-divider"></div>

  <!-- History group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      title="Undo (Cmd+Z)"
      onclick={() => editor?.chain().focus().undo().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="1 4 1 10 7 10"></polyline>
        <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"></path>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      title="Redo (Cmd+Shift+Z)"
      onclick={() => editor?.chain().focus().redo().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="23 4 23 10 17 10"></polyline>
        <path d="M20.49 15a9 9 0 1 1-2.13-9.36L23 10"></path>
      </svg>
    </button>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Page size group -->
  <div class="toolbar-group">
    <select
      class="toolbar-select"
      title="Page size"
      onchange={setPageSize}
      value={appState.pageSize}
    >
      <option value="letter">US Letter</option>
      <option value="a4">A4</option>
      <option value="legal">Legal</option>
    </select>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Block group -->
  <div class="toolbar-group">
    <select
      class="toolbar-select"
      title="Block type"
      onchange={setBlockType}
      value={currentBlockType()}
      disabled={!editor}
    >
      <option value="paragraph">Paragraph</option>
      <option value="h1">Heading 1</option>
      <option value="h2">Heading 2</option>
      <option value="h3">Heading 3</option>
      <option value="blockquote">Blockquote</option>
    </select>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Font family / size / color group -->
  <div class="toolbar-group">
    <select
      class="toolbar-select toolbar-select--font-family"
      title="Font family"
      onchange={setFontFamily}
      value={currentFontFamily()}
      disabled={!editor}
    >
      {#each FONT_FAMILIES as family}
        <option value={family}>{family}</option>
      {/each}
    </select>

    <select
      class="toolbar-select toolbar-select--font-size"
      title="Font size"
      onchange={setFontSize}
      value={currentFontSize()}
      disabled={!editor}
    >
      {#each FONT_SIZES as size}
        <option value={size}>{size}</option>
      {/each}
    </select>

    <div class="color-btn-wrap" title="Text color">
      <input
        type="color"
        class="toolbar-color-input"
        value={currentColor()}
        oninput={setColor}
        disabled={!editor}
      />
    </div>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Inline group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      class:active={isActive('bold')}
      title="Bold (Cmd+B)"
      onclick={() => editor?.chain().focus().toggleBold().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M6 4h8a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"></path>
        <path d="M6 12h9a4 4 0 0 1 4 4 4 4 0 0 1-4 4H6z"></path>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      class:active={isActive('italic')}
      title="Italic (Cmd+I)"
      onclick={() => editor?.chain().focus().toggleItalic().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="19" y1="4" x2="10" y2="4"></line>
        <line x1="14" y1="20" x2="5" y2="20"></line>
        <line x1="15" y1="4" x2="9" y2="20"></line>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      class:active={isActive('underline')}
      title="Underline (Cmd+U)"
      onclick={() => editor?.chain().focus().toggleUnderline().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M6 3v7a6 6 0 0 0 6 6 6 6 0 0 0 6-6V3"></path>
        <line x1="4" y1="21" x2="20" y2="21"></line>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      class:active={isActive('strike')}
      title="Strikethrough"
      onclick={() => editor?.chain().focus().toggleStrike().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M17.3 4.9c-2.3-.6-4.4-1-6.2-.9-2.7 0-5.3.7-5.3 3.6 0 1.5 1.8 3.3 7.2 3.3"></path>
        <line x1="4" y1="12" x2="20" y2="12"></line>
        <path d="M17.3 12c1.9.2 4 1.2 4 3.6s-2.6 3.6-5.3 3.6c-1.8 0-3.9-.3-6.2-.9"></path>
      </svg>
    </button>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Alignment group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      class:active={isActive('paragraph', { textAlign: 'left' }) || isActive('heading', { textAlign: 'left' })}
      title="Align left"
      onclick={() => editor?.chain().focus().setTextAlign('left').run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="17" y1="10" x2="3" y2="10"></line>
        <line x1="21" y1="6" x2="3" y2="6"></line>
        <line x1="21" y1="14" x2="3" y2="14"></line>
        <line x1="17" y1="18" x2="3" y2="18"></line>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      class:active={isActive('paragraph', { textAlign: 'center' }) || isActive('heading', { textAlign: 'center' })}
      title="Align center"
      onclick={() => editor?.chain().focus().setTextAlign('center').run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="10" x2="6" y2="10"></line>
        <line x1="21" y1="6" x2="3" y2="6"></line>
        <line x1="21" y1="14" x2="3" y2="14"></line>
        <line x1="18" y1="18" x2="6" y2="18"></line>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      class:active={isActive('paragraph', { textAlign: 'right' }) || isActive('heading', { textAlign: 'right' })}
      title="Align right"
      onclick={() => editor?.chain().focus().setTextAlign('right').run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="21" y1="10" x2="7" y2="10"></line>
        <line x1="21" y1="6" x2="3" y2="6"></line>
        <line x1="21" y1="14" x2="3" y2="14"></line>
        <line x1="21" y1="18" x2="7" y2="18"></line>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      class:active={isActive('paragraph', { textAlign: 'justify' }) || isActive('heading', { textAlign: 'justify' })}
      title="Justify"
      onclick={() => editor?.chain().focus().setTextAlign('justify').run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="21" y1="10" x2="3" y2="10"></line>
        <line x1="21" y1="6" x2="3" y2="6"></line>
        <line x1="21" y1="14" x2="3" y2="14"></line>
        <line x1="21" y1="18" x2="3" y2="18"></line>
      </svg>
    </button>
  </div>

  <div class="toolbar-divider"></div>

  <!-- List group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      class:active={isActive('bulletList')}
      title="Bullet list"
      onclick={() => editor?.chain().focus().toggleBulletList().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="8" y1="6" x2="21" y2="6"></line>
        <line x1="8" y1="12" x2="21" y2="12"></line>
        <line x1="8" y1="18" x2="21" y2="18"></line>
        <line x1="3" y1="6" x2="3.01" y2="6"></line>
        <line x1="3" y1="12" x2="3.01" y2="12"></line>
        <line x1="3" y1="18" x2="3.01" y2="18"></line>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      class:active={isActive('orderedList')}
      title="Numbered list"
      onclick={() => editor?.chain().focus().toggleOrderedList().run()}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="10" y1="6" x2="21" y2="6"></line>
        <line x1="10" y1="12" x2="21" y2="12"></line>
        <line x1="10" y1="18" x2="21" y2="18"></line>
        <path d="M4 6h1v4"></path>
        <path d="M4 10h2"></path>
        <path d="M6 18H4c0-1 2-2 2-3s-1-1.5-2-1"></path>
      </svg>
    </button>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Indent group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      title="Decrease indent"
      onclick={() => {
        if (!editor) return;
        const inList = editor.isActive('listItem');
        if (inList) {
          editor.chain().focus().liftListItem('listItem').run();
        } else {
          editor.chain().focus().outdent().run();
        }
      }}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="21" y1="6" x2="9" y2="6"></line>
        <line x1="21" y1="12" x2="9" y2="12"></line>
        <line x1="21" y1="18" x2="9" y2="18"></line>
        <polyline points="3 8 1 10 3 12"></polyline>
      </svg>
    </button>
    <button
      class="toolbar-btn"
      title="Increase indent"
      onclick={() => {
        if (!editor) return;
        const inList = editor.isActive('listItem');
        if (inList) {
          editor.chain().focus().sinkListItem('listItem').run();
        } else {
          editor.chain().focus().indent().run();
        }
      }}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="21" y1="6" x2="9" y2="6"></line>
        <line x1="21" y1="12" x2="9" y2="12"></line>
        <line x1="21" y1="18" x2="9" y2="18"></line>
        <polyline points="1 8 3 10 1 12"></polyline>
      </svg>
    </button>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Insert group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      class:active={isActive('link')}
      title="Insert link"
      onclick={insertLink}
      disabled={!editor}
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path>
        <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>
      </svg>
    </button>
  </div>

  <!-- Spacer pushes AI + Settings to the right -->
  <div class="toolbar-spacer" aria-hidden="true"></div>

  <!-- AI group -->
  <div class="toolbar-group">
    <span class="ai-btn-wrap">
      <button
        class="toolbar-btn"
        class:active={appState.aiAvailable && appState.aiEnabled}
        title={appState.aiAvailable
          ? `AI autocomplete - ${appState.aiActiveProvider} (Cmd+Shift+A)`
          : 'AI autocomplete (configure API key in Settings)'}
        onclick={() => onToggleAi?.()}
        disabled={!appState.aiAvailable}
      >
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2L13.5 8.5L20 10L13.5 11.5L12 18L10.5 11.5L4 10L10.5 8.5Z"></path>
          <path d="M19 3L19.5 5L21.5 5.5L19.5 6L19 8L18.5 6L16.5 5.5L18.5 5Z"></path>
          <path d="M5 16L5.5 18L7.5 18.5L5.5 19L5 21L4.5 19L2.5 18.5L4.5 18Z"></path>
        </svg>
      </button>
      <AiStatusDot badge />
    </span>
  </div>

  <div class="toolbar-divider"></div>

  <!-- Settings group -->
  <div class="toolbar-group">
    <button
      class="toolbar-btn"
      class:active={appState.settingsOpen}
      title="Settings"
      onclick={openSettings}
      aria-label="Open settings"
    >
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3"></circle>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
      </svg>
    </button>
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    height: 44px;
    padding: 0 10px;
    background-color: var(--panel-bg);
    border-bottom: 1px solid var(--border-color);
    gap: 2px;
    flex-shrink: 0;
    font-family: system-ui, -apple-system, sans-serif;
    -webkit-app-region: no-drag;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }

  .toolbar::-webkit-scrollbar {
    display: none;
  }

  .toolbar-spacer {
    flex: 1;
    min-width: 8px;
  }

  .toolbar-group {
    display: flex;
    align-items: center;
    gap: 1px;
    flex-shrink: 0;
  }

  .ai-btn-wrap {
    position: relative;
    display: inline-flex;
  }

  .toolbar-divider {
    width: 1px;
    height: 20px;
    background-color: var(--border-color);
    margin: 0 3px;
    flex-shrink: 0;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 5px;
    background: transparent;
    color: var(--fg);
    cursor: pointer;
    padding: 0;
    transition: background-color 0.1s, color 0.1s;
    opacity: 0.75;
  }

  .toolbar-btn:hover:not(:disabled) {
    background-color: var(--button-hover-bg);
    opacity: 1;
  }

  .toolbar-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .toolbar-btn.active {
    background-color: var(--button-bg);
    opacity: 1;
  }

  .toolbar-select {
    height: 28px;
    padding: 0 7px;
    border: 1px solid var(--border-color);
    border-radius: 5px;
    background-color: var(--input-bg);
    color: var(--fg);
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 12px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.1s;
  }

  .toolbar-select:hover:not(:disabled) {
    border-color: var(--input-border);
  }

  .toolbar-select:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .toolbar-select--font-family {
    width: 128px;
  }

  .toolbar-select--font-size {
    width: 56px;
  }

  .color-btn-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .toolbar-color-input {
    width: 30px;
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: 5px;
    padding: 2px;
    background: var(--input-bg);
    cursor: pointer;
  }

  .toolbar-color-input:disabled {
    opacity: 0.35;
    cursor: default;
  }
</style>
