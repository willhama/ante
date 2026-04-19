<script lang="ts">
  import { appState } from '$lib/state/app-state.svelte';

  function formatCount(n: number): string {
    if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
    return String(n);
  }
</script>

<div class="status-bar" role="status" aria-label="Document status">
  <!-- Left: word + char count -->
  <div class="status-group status-group--left">
    <span class="status-item" title="Word count">
      {formatCount(appState.wordCount)}
      <span class="status-label">w</span>
    </span>
    <span class="status-sep" aria-hidden="true">·</span>
    <span class="status-item" title="Character count (no spaces)">
      {formatCount(appState.charCount)}
      <span class="status-label">ch</span>
    </span>
  </div>

  <!-- Center: page X of Y -->
  <div class="status-group status-group--center">
    <span class="status-item" title="Current page">
      Page {appState.currentPage} of {appState.totalPages}
    </span>
  </div>

  <!-- Right: dirty indicator -->
  <div class="status-group status-group--right">
    {#if appState.isDirty}
      <span class="status-item status-item--dirty" title="Unsaved changes">
        <svg width="6" height="6" viewBox="0 0 6 6" aria-hidden="true">
          <circle cx="3" cy="3" r="3" fill="currentColor"/>
        </svg>
        Unsaved
      </span>
    {:else}
      <span class="status-item status-item--saved" title="All changes saved">
        Saved
      </span>
    {/if}
  </div>
</div>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    height: 24px;
    padding: 0 12px;
    background-color: var(--panel-bg);
    border-top: 1px solid var(--border-color);
    flex-shrink: 0;
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 11px;
    color: var(--gutter-fg);
    user-select: none;
    -webkit-user-select: none;
    -webkit-app-region: no-drag;
  }

  .status-group {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .status-group--left {
    flex: 1;
    justify-content: flex-start;
  }

  .status-group--center {
    flex: 1;
    justify-content: center;
  }

  .status-group--right {
    flex: 1;
    justify-content: flex-end;
  }

  .status-item {
    display: flex;
    align-items: center;
    gap: 2px;
    letter-spacing: 0.01em;
  }

  .status-label {
    opacity: 0.6;
  }

  .status-sep {
    opacity: 0.4;
  }

  .status-item--dirty {
    color: oklch(0.65 0.15 50);
    gap: 4px;
  }

  :global([data-theme="dark"]) .status-item--dirty {
    color: oklch(0.75 0.12 50);
  }

  .status-item--saved {
    color: var(--gutter-fg);
  }
</style>
