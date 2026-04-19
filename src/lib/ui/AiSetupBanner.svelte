<script lang="ts">
  import { appState } from '$lib/state/app-state.svelte';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import X from '@lucide/svelte/icons/x';

  let dismissed = $state(false);

  const visible = $derived(!appState.aiAvailable && !dismissed);

  function openSettings(): void {
    appState.settingsOpen = true;
  }

  function dismiss(e: MouseEvent): void {
    e.stopPropagation();
    dismissed = true;
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      openSettings();
    }
  }
</script>

{#if visible}
  <div
    role="button"
    tabindex="0"
    class="ai-setup-banner"
    onclick={openSettings}
    onkeydown={onKeydown}
    aria-label="AI autocomplete not configured. Click to open settings."
  >
    <div class="icon">
      <Sparkles size={16} />
    </div>
    <div class="text">
      <div class="title">AI autocomplete not configured</div>
      <div class="hint">Click to add an API key</div>
    </div>
    <button
      type="button"
      class="dismiss"
      onclick={dismiss}
      aria-label="Dismiss"
    >
      <X size={14} />
    </button>
  </div>
{/if}

<style>
  .ai-setup-banner {
    position: fixed;
    top: 72px;
    right: 16px;
    z-index: 50;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px 10px 14px;
    background: var(--popover, #fff);
    color: var(--popover-foreground, #111);
    border: 1px solid var(--border, rgba(0, 0, 0, 0.08));
    border-radius: 10px;
    box-shadow:
      0 8px 24px -10px rgba(0, 0, 0, 0.18),
      0 2px 6px -2px rgba(0, 0, 0, 0.08);
    font-size: 13px;
    line-height: 1.25;
    cursor: pointer;
    max-width: 280px;
    transition: transform 120ms ease, box-shadow 120ms ease;
  }

  .ai-setup-banner:hover {
    transform: translateY(-1px);
    box-shadow:
      0 10px 28px -10px rgba(0, 0, 0, 0.24),
      0 3px 8px -2px rgba(0, 0, 0, 0.1);
  }

  .ai-setup-banner:focus-visible {
    outline: 2px solid var(--ring, #6366f1);
    outline-offset: 2px;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--primary, #6366f1) 12%, transparent);
    color: var(--primary, #6366f1);
    flex-shrink: 0;
  }

  .text {
    flex: 1;
    min-width: 0;
  }

  .title {
    font-weight: 600;
  }

  .hint {
    color: var(--muted-foreground, #6b7280);
    font-size: 12px;
    margin-top: 1px;
  }

  .dismiss {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--muted-foreground, #6b7280);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: background 120ms ease, color 120ms ease;
  }

  .dismiss:hover {
    background: var(--accent, rgba(0, 0, 0, 0.06));
    color: var(--foreground, #111);
  }
</style>
