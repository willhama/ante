<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { appState } from '$lib/state/app-state.svelte';

  interface Props {
    badge?: boolean;
  }
  const { badge = false }: Props = $props();

  interface AiTestResult {
    ok: boolean;
    model_count: number | null;
    error: string | null;
  }

  type Status = 'no-key' | 'unknown' | 'checking' | 'ok' | 'error';

  const POLL_MS = 60_000;

  let status = $state<Status>('unknown');
  let lastError = $state<string | null>(null);
  let lastCheckedAt = $state<number | null>(null);
  let inFlight = false;

  async function check(): Promise<void> {
    if (!appState.aiAvailable) {
      status = 'no-key';
      lastError = null;
      return;
    }
    if (inFlight) return;
    inFlight = true;
    const prev = status;
    status = prev === 'unknown' ? 'checking' : prev;
    try {
      const result = await invoke<AiTestResult>('test_ai_config', {
        payload: { provider: appState.aiActiveProvider },
      });
      lastCheckedAt = Date.now();
      if (result.ok) {
        status = 'ok';
        lastError = null;
      } else {
        status = 'error';
        lastError = result.error ?? 'request failed';
      }
    } catch (e) {
      status = 'error';
      lastError = e instanceof Error ? e.message : 'check failed';
      lastCheckedAt = Date.now();
    } finally {
      inFlight = false;
    }
  }

  function formatTime(ts: number | null): string {
    if (ts === null) return 'never';
    const diff = Date.now() - ts;
    if (diff < 10_000) return 'just now';
    if (diff < 60_000) return `${Math.floor(diff / 1000)}s ago`;
    if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
    return new Date(ts).toLocaleTimeString();
  }

  const tooltip = $derived.by(() => {
    if (status === 'no-key') return 'No API key configured';
    if (status === 'unknown' || status === 'checking') return 'Checking API key...';
    const when = formatTime(lastCheckedAt);
    if (status === 'ok') return `API key OK - ${appState.aiActiveProvider} (checked ${when})`;
    return `API error: ${lastError ?? 'unknown'} (checked ${when})`;
  });

  // Re-check when the active provider changes or aiAvailable flips.
  $effect(() => {
    const _ = appState.aiActiveProvider;
    const __ = appState.aiAvailable;
    status = 'unknown';
    lastError = null;
    check();
  });

  // Re-check when settings dialog closes (user may have saved a new key).
  let prevSettingsOpen = appState.settingsOpen;
  $effect(() => {
    const open = appState.settingsOpen;
    if (prevSettingsOpen && !open) check();
    prevSettingsOpen = open;
  });

  onMount(() => {
    const onVisibility = (): void => {
      if (document.visibilityState === 'visible') check();
    };
    document.addEventListener('visibilitychange', onVisibility);

    const interval = setInterval(() => {
      if (document.visibilityState === 'visible') check();
    }, POLL_MS);

    return () => {
      clearInterval(interval);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  });
</script>

<span
  class="dot"
  class:badge
  data-status={status}
  title={tooltip}
  role="status"
  aria-label={tooltip}
></span>

<style>
  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 999px;
    margin: 0 6px;
    flex-shrink: 0;
    transition: background-color 200ms ease, box-shadow 200ms ease;
  }

  .dot.badge {
    position: absolute;
    bottom: 2px;
    right: 2px;
    width: 8px;
    height: 8px;
    margin: 0;
    border: 2px solid var(--background, #fff);
    box-shadow: none;
    pointer-events: none;
    z-index: 1;
  }

  .dot[data-status='ok'] {
    background: #22c55e;
    box-shadow: 0 0 0 2px color-mix(in srgb, #22c55e 25%, transparent);
  }

  .dot[data-status='error'] {
    background: #ef4444;
    box-shadow: 0 0 0 2px color-mix(in srgb, #ef4444 25%, transparent);
  }

  .dot[data-status='no-key'] {
    background: var(--muted-foreground, #9ca3af);
    opacity: 0.5;
  }

  .dot[data-status='unknown'],
  .dot[data-status='checking'] {
    background: #f59e0b;
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50%      { opacity: 0.4; }
  }
</style>
