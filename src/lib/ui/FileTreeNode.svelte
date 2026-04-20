<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Self from './FileTreeNode.svelte';
  import { appState } from '$lib/state/app-state.svelte';

  interface DirEntry {
    name: string;
    path: string;
    is_dir: boolean;
  }

  interface Props {
    entry: DirEntry;
    depth: number;
    expanded: Map<string, boolean>;
    cache: Map<string, DirEntry[]>;
    loading: Set<string>;
    onOpenFile: (path: string) => void;
    onCreateInFolder: (folderPath: string) => void;
    onMoveFile: (src: string, dstDir: string) => void;
  }

  const {
    entry,
    depth,
    expanded,
    cache,
    loading,
    onOpenFile,
    onCreateInFolder,
    onMoveFile,
  }: Props = $props();

  let dragOver = $state(false);

  function handleDragStart(e: DragEvent): void {
    if (entry.is_dir) return;
    if (!e.dataTransfer) return;
    e.dataTransfer.setData('application/x-ante-path', entry.path);
    e.dataTransfer.effectAllowed = 'move';
  }

  function handleDragOver(e: DragEvent): void {
    if (!entry.is_dir) return;
    if (!e.dataTransfer?.types.includes('application/x-ante-path')) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    dragOver = true;
  }

  function handleDragLeave(): void {
    dragOver = false;
  }

  function handleDrop(e: DragEvent): void {
    if (!entry.is_dir) return;
    const src = e.dataTransfer?.getData('application/x-ante-path');
    dragOver = false;
    if (!src) return;
    e.preventDefault();
    if (src === entry.path) return;
    onMoveFile(src, entry.path);
  }

  const OPENABLE_EXTS = new Set([
    'html', 'htm', 'txt', 'md', 'markdown', 'rst', 'log',
  ]);

  function isOpenable(name: string): boolean {
    const dot = name.lastIndexOf('.');
    if (dot === -1) return false;
    return OPENABLE_EXTS.has(name.slice(dot + 1).toLowerCase());
  }

  const isExpanded = $derived(expanded.get(entry.path) ?? false);
  const isLoading = $derived(loading.has(entry.path));
  const isActive = $derived(!entry.is_dir && entry.path === appState.filePath);

  const children = $derived.by(() => {
    const raw = cache.get(entry.path);
    if (!raw) return null;
    return raw.filter((c) => c.is_dir || isOpenable(c.name));
  });

  async function toggle(): Promise<void> {
    if (!entry.is_dir) return;
    const willExpand = !isExpanded;
    expanded.set(entry.path, willExpand);
    if (willExpand && !cache.has(entry.path) && !loading.has(entry.path)) {
      loading.add(entry.path);
      try {
        const result = await invoke<DirEntry[]>('list_directory', {
          path: entry.path,
        });
        cache.set(entry.path, result);
      } catch {
        cache.set(entry.path, []);
      } finally {
        loading.delete(entry.path);
      }
    }
  }

  function handleClick(): void {
    if (entry.is_dir) {
      toggle();
    } else if (entry.path !== appState.filePath) {
      onOpenFile(entry.path);
    }
  }
</script>

<button
  type="button"
  class="row"
  class:active={isActive}
  class:dir={entry.is_dir}
  class:drop-target={dragOver}
  style="padding-left: {8 + depth * 14}px"
  draggable={!entry.is_dir}
  onclick={handleClick}
  ondragstart={handleDragStart}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  title={entry.path}
>
  {#if entry.is_dir}
    <span class="chevron" class:open={isExpanded} aria-hidden="true">
      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="9 18 15 12 9 6"></polyline>
      </svg>
    </span>
  {:else}
    <span class="chevron-spacer" aria-hidden="true"></span>
  {/if}
  <span class="row-icon" aria-hidden="true">
    {#if entry.is_dir}
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
      </svg>
    {:else}
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
        <polyline points="14 2 14 8 20 8"></polyline>
      </svg>
    {/if}
  </span>
  <span class="row-name">{entry.name}</span>
</button>

{#if entry.is_dir && isExpanded}
  {#if isLoading && !children}
    <div class="loading" style="padding-left: {8 + (depth + 1) * 14 + 12}px">Loading…</div>
  {:else if children}
    {#if children.length > 0}
      {#each children as child (child.path)}
        <Self
          entry={child}
          depth={depth + 1}
          {expanded}
          {cache}
          {loading}
          {onOpenFile}
          {onCreateInFolder}
          {onMoveFile}
        />
      {/each}
    {/if}
    <button
      type="button"
      class="add-row"
      style="padding-left: {8 + (depth + 1) * 14 + 12 + 6}px"
      onclick={() => onCreateInFolder(entry.path)}
      title="Add document to {entry.name}"
    >
      <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <line x1="12" y1="5" x2="12" y2="19"></line>
        <line x1="5" y1="12" x2="19" y2="12"></line>
      </svg>
      <span>New document</span>
    </button>
  {/if}
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 4px 12px 4px 8px;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    border-radius: 0;
  }

  .row:hover {
    background: var(--accent, rgba(0, 0, 0, 0.06));
  }

  .row.active {
    background: color-mix(in srgb, var(--primary, #6366f1) 14%, transparent);
    color: var(--primary, #6366f1);
    font-weight: 600;
  }

  .row.dir {
    color: var(--foreground, #111);
  }

  .row.drop-target {
    background: color-mix(in srgb, var(--primary, #6366f1) 18%, transparent);
    outline: 1px dashed var(--primary, #6366f1);
    outline-offset: -2px;
  }

  .chevron {
    display: grid;
    place-items: center;
    width: 12px;
    height: 12px;
    color: var(--muted-foreground, #6b7280);
    transition: transform 120ms ease;
    flex-shrink: 0;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .chevron-spacer {
    display: inline-block;
    width: 12px;
    height: 12px;
    flex-shrink: 0;
  }

  .row-icon {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    color: var(--muted-foreground, #6b7280);
  }

  .row.active .row-icon {
    color: var(--primary, #6366f1);
  }

  .row-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .loading {
    font-size: 12px;
    color: var(--muted-foreground, #6b7280);
    padding-top: 2px;
    padding-bottom: 4px;
  }

  .add-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 3px 12px;
    border: none;
    background: transparent;
    color: var(--muted-foreground, #6b7280);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .add-row:hover {
    background: var(--accent, rgba(0, 0, 0, 0.06));
    color: var(--foreground, #111);
  }
</style>
