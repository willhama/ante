<script lang="ts">
  import { recentFiles, filenameFromPath, dirFromPath } from '$lib/state/recent-files.svelte';
  import { appState } from '$lib/state/app-state.svelte';

  interface Props {
    onNew: () => void;
    onOpen: () => void;
    onOpenPath: (path: string) => void;
  }

  let { onNew, onOpen, onOpenPath }: Props = $props();

  const isMac = typeof navigator !== 'undefined' && /mac/i.test(navigator.platform);
  const mod = isMac ? '\u2318' : 'Ctrl';

  function handleRemove(event: MouseEvent, path: string): void {
    event.stopPropagation();
    recentFiles.remove(path);
  }
</script>

<div class="welcome" class:dark={appState.resolvedTheme === 'dark'}>
  <div class="welcome-inner">
    <h1 class="title">ante</h1>
    <p class="tagline">A calm place to write.</p>

    <section class="column">
      <h2 class="section-label">Start</h2>
      <button class="row action" onclick={onNew}>
        <span class="label">New file</span>
        <span class="shortcut">{mod} N</span>
      </button>
      <button class="row action" onclick={onOpen}>
        <span class="label">Open file...</span>
        <span class="shortcut">{mod} O</span>
      </button>
    </section>

    <section class="column">
      <h2 class="section-label">Recent</h2>
      {#if recentFiles.list.length === 0}
        <p class="empty">No recent files.</p>
      {:else}
        {#each recentFiles.list as item (item.path)}
          <button class="row recent" onclick={() => onOpenPath(item.path)} title={item.path}>
            <span class="label">{filenameFromPath(item.path)}</span>
            <span class="dir">{dirFromPath(item.path)}</span>
            <span
              class="remove"
              role="button"
              tabindex="0"
              aria-label="Remove from recents"
              title="Remove from recents"
              onclick={(e) => handleRemove(e, item.path)}
              onkeydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  recentFiles.remove(item.path);
                }
              }}
            >×</span>
          </button>
        {/each}
      {/if}
    </section>
  </div>
</div>

<style>
  .welcome {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--canvas-bg, #f6f5f2);
    color: var(--fg, #1a1a1a);
    overflow: auto;
    padding: 48px 24px;
  }

  .welcome.dark {
    background: var(--canvas-bg, #141414);
    color: var(--fg, #e6e6e6);
  }

  .welcome-inner {
    width: 100%;
    max-width: 520px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .title {
    font-family: 'Inter Variable', system-ui, sans-serif;
    font-size: 44px;
    font-weight: 300;
    letter-spacing: -0.02em;
    margin: 0;
    line-height: 1;
  }

  .tagline {
    margin: -16px 0 0;
    font-size: 14px;
    opacity: 0.55;
  }

  .column {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-label {
    font-size: 11px;
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    opacity: 0.5;
    margin: 0 0 6px;
  }

  .row {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 6px;
    background: transparent;
    border: none;
    color: inherit;
    text-align: left;
    font: inherit;
    cursor: pointer;
    width: 100%;
    transition: background-color 0.12s ease;
  }

  .row:hover,
  .row:focus-visible {
    background: color-mix(in srgb, currentColor 8%, transparent);
    outline: none;
  }

  .row.action .label {
    font-size: 15px;
  }

  .row.recent {
    position: relative;
  }

  .label {
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
    max-width: 240px;
  }

  .dir {
    font-size: 12px;
    opacity: 0.5;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    direction: rtl;
    text-align: left;
  }

  .shortcut {
    margin-left: auto;
    font-size: 12px;
    opacity: 0.45;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.04em;
  }

  .empty {
    font-size: 13px;
    opacity: 0.5;
    margin: 4px 10px;
  }

  .remove {
    margin-left: 8px;
    opacity: 0;
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    flex-shrink: 0;
    transition: opacity 0.12s ease, background-color 0.12s ease;
  }

  .row.recent:hover .remove,
  .row.recent:focus-within .remove {
    opacity: 0.6;
  }

  .remove:hover {
    opacity: 1 !important;
    background: color-mix(in srgb, currentColor 12%, transparent);
  }
</style>
