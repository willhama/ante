<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { SvelteMap, SvelteSet } from 'svelte/reactivity';
  import { appState } from '$lib/state/app-state.svelte';
  import FileTreeNode from './FileTreeNode.svelte';

  interface Props {
    onOpenFile: (path: string) => void;
  }
  const { onOpenFile }: Props = $props();

  interface DirEntry {
    name: string;
    path: string;
    is_dir: boolean;
  }

  const OPENABLE_EXTS = new Set([
    'html', 'htm', 'txt', 'md', 'markdown', 'rst', 'log',
  ]);

  const SEP_RX = /[\\/]/;

  function parentDir(path: string): string {
    const lastIdx = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return lastIdx > 0 ? path.slice(0, lastIdx) : path;
  }

  function basename(path: string): string {
    const parts = path.split(SEP_RX);
    return parts[parts.length - 1] || path;
  }

  function isOpenable(name: string): boolean {
    const dot = name.lastIndexOf('.');
    if (dot === -1) return false;
    return OPENABLE_EXTS.has(name.slice(dot + 1).toLowerCase());
  }

  let rootEntries = $state<DirEntry[]>([]);
  let rootPath = $state<string>('');
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);

  // Shared expansion + child cache used by every TreeNode.
  const expanded = new SvelteMap<string, boolean>();
  const cache = new SvelteMap<string, DirEntry[]>();
  const loadingSet = new SvelteSet<string>();

  async function refreshRoot(targetPath: string): Promise<void> {
    if (!targetPath) {
      rootEntries = [];
      rootPath = '';
      errorMsg = null;
      expanded.clear();
      cache.clear();
      loadingSet.clear();
      return;
    }
    loading = true;
    errorMsg = null;
    try {
      const result = await invoke<DirEntry[]>('list_directory', { path: targetPath });
      rootEntries = result;
      rootPath = parentDir(targetPath) || targetPath;
      cache.set(rootPath, result);
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : 'Failed to read directory';
      rootEntries = [];
    } finally {
      loading = false;
    }
  }

  // Re-read root when active file's parent dir changes (not when file itself changes inside same dir).
  let lastRootPath = '';
  $effect(() => {
    const fp = appState.filePath ?? '';
    const newRoot = fp ? parentDir(fp) : '';
    if (newRoot !== lastRootPath) {
      lastRootPath = newRoot;
      refreshRoot(fp);
    }
  });

  const visibleRoot = $derived(
    rootEntries.filter((e) => e.is_dir || isOpenable(e.name)),
  );

  onMount(() => {
    if (appState.filePath) {
      refreshRoot(appState.filePath);
    }
  });

  async function createDocument(): Promise<void> {
    if (!rootPath) return;
    try {
      const result = await invoke<{ path: string }>('create_document', {
        dir: rootPath,
      });
      onOpenFile(result.path);
      // The new file lives in the same root; the file-change effect won't
      // refresh because parentDir is unchanged. Force a re-read so it appears.
      cache.delete(rootPath);
      await refreshRoot(result.path);
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : 'Failed to create document';
    }
  }
</script>

<aside class="file-explorer" style="width: {appState.sidebarWidth}px">
  <header class="header">
    <span class="header-title" title={rootPath}>
      {rootPath ? basename(rootPath) || rootPath : 'No folder'}
    </span>
    <button
      type="button"
      class="icon-btn"
      title="New document in this folder"
      onclick={createDocument}
      disabled={!rootPath}
      aria-label="New document"
    >
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="12" y1="5" x2="12" y2="19"></line>
        <line x1="5" y1="12" x2="19" y2="12"></line>
      </svg>
    </button>
    <button
      type="button"
      class="icon-btn"
      title="Refresh"
      onclick={() => {
        cache.clear();
        expanded.clear();
        loadingSet.clear();
        refreshRoot(appState.filePath ?? '');
      }}
      aria-label="Refresh"
    >
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="23 4 23 10 17 10"></polyline>
        <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"></path>
      </svg>
    </button>
  </header>

  <div class="body">
    {#if !appState.filePath}
      <p class="empty">Open a file to see its folder here.</p>
    {:else if loading && rootEntries.length === 0}
      <p class="empty">Loading…</p>
    {:else if errorMsg}
      <p class="empty error">{errorMsg}</p>
    {:else if visibleRoot.length === 0}
      <p class="empty">No openable files in this folder.</p>
    {:else}
      <div class="tree">
        {#each visibleRoot as entry (entry.path)}
          <FileTreeNode
            {entry}
            depth={0}
            {expanded}
            {cache}
            loading={loadingSet}
            {onOpenFile}
          />
        {/each}
      </div>
    {/if}
  </div>
</aside>

<style>
  .file-explorer {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-right: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
    background: var(--sidebar-bg, var(--background, #f7f7f7));
    color: var(--foreground, #111);
    flex-shrink: 0;
    overflow: hidden;
    font-size: 13px;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border-color, rgba(0, 0, 0, 0.08));
  }

  .header-title {
    font-weight: 600;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.4px;
    color: var(--muted-foreground, #6b7280);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: none;
    background: transparent;
    color: var(--muted-foreground, #6b7280);
    border-radius: 4px;
    cursor: pointer;
    padding: 0;
  }

  .icon-btn:hover {
    background: var(--accent, rgba(0, 0, 0, 0.06));
    color: var(--foreground, #111);
  }

  .body {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: none;
  }

  .body::-webkit-scrollbar {
    display: none;
  }

  .tree {
    padding: 4px 0;
  }

  .empty {
    padding: 14px 12px;
    color: var(--muted-foreground, #6b7280);
    font-size: 12px;
    line-height: 1.4;
  }

  .empty.error {
    color: #ef4444;
  }
</style>
