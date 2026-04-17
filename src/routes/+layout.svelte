<script lang="ts">
  import { onMount } from 'svelte';
  import '../app.css';
  import { appState } from '$lib/state/app-state.svelte';
  import { initThemeDetection, destroyThemeDetection, applyThemeToDocument } from '$lib/state/theme';

  let { children } = $props();

  onMount(() => {
    initThemeDetection();
    return () => {
      destroyThemeDetection();
    };
  });

  // Reactively apply theme to document root
  $effect(() => {
    applyThemeToDocument(appState.resolvedTheme);
  });

  // Reactively update window title
  $effect(() => {
    const title = appState.windowTitle;
    updateWindowTitle(title);
  });

  async function updateWindowTitle(title: string) {
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      await getCurrentWindow().setTitle(title);
    } catch {
      // Fallback for non-Tauri environment (dev browser)
      document.title = title;
    }
  }
</script>

{@render children()}
