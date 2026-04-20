<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { appState } from '$lib/state/app-state.svelte';

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

  let entries = $state<DirEntry[]>([]);
  let dirPath = $state<string>('');
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);

  const SEP_RX = /[\\/]/;

  function parentDir(path: string): string {
    const idx = path.search(SEP_RX);
    if (idx === -1) return '';
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

  async function refresh(targetPath: string): Promise<void> {
    if (!targetPath) {
      entries = [];
      dirPath = '';
      errorMsg = null;
      return;
    }
    loading = true;
    errorMsg = null;
    try {
      const result = await invoke<DirEntry[]>('list_directory', { path: targetPath });
      entries = result;
      dirPath = parentDir(targetPath) || targetPath;
    } catch (e) {
      errorMsg = e instanceof Error ? e.message : 'Failed to read directory';
      entries = [];
    } finally {
      loading = false;
    }
  }

  // Re-read when active file changes.
  $effect(() => {
    refresh(appState.filePath ?? '');
  });

  const visibleEntries = $derived(
    entries.filter((e) => e.is_dir || isOpenable(e.name)),
  );

  const activeFilename = $derived(
    appState.filePath ? basename(appState.filePath) : null,
  );

  function handleClick(entry: DirEntry): void {
    if (entry.is_dir) return;
    if (entry.path === appState.filePath) return;
    onOpenFile(entry.path);
  }

  onMount(() => {
    refresh(appState.filePath ?? '');
  });
</script>

<aside class="file-explorer" style="width: {appState.sidebarWidth}px">
  <header class="header">
    <span class="header-title" title={dirPath}>
      {dirPath ? basename(dirPath) || dirPath : 'No folder'}
    </span>
    <button
      type="button"
      class="icon-btn"
      title="Refresh"
      onclick={() => refresh(appState.filePath ?? '')}
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
    {:else if loading && entries.length === 0}
      <p class="empty">Loading…</p>
    {:else if errorMsg}
      <p class="empty error">{errorMsg}</p>
    {:else if visibleEntries.length === 0}
      <p class="empty">No openable files in this folder.</p>
    {:else}
      <ul class="list">
        {#each visibleEntries as entry (entry.path)}
          {@const isActive = entry.name === activeFilename}
          <li>
            <button
              type="button"
              class="row"
              class:active={isActive}
              class:dir={entry.is_dir}
              disabled={entry.is_dir}
              onclick={() => handleClick(entry)}
              title={entry.path}
            >
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
          </li>
        {/each}
      </ul>
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
    background: transparent;
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

  .empty {
    padding: 14px 12px;
    color: var(--muted-foreground, #6b7280);
    font-size: 12px;
    line-height: 1.4;
  }

  .empty.error {
    color: #ef4444;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 4px 0;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 12px;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    border-radius: 0;
  }

  .row:hover:not(:disabled) {
    background: var(--accent, rgba(0, 0, 0, 0.06));
  }

  .row.active {
    background: color-mix(in srgb, var(--primary, #6366f1) 14%, transparent);
    color: var(--primary, #6366f1);
    font-weight: 600;
  }

  .row.dir {
    color: var(--muted-foreground, #6b7280);
    cursor: default;
  }

  .row:disabled {
    cursor: default;
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
</style>
