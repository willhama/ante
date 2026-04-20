<script lang="ts">
  import { onMount } from 'svelte';
  import type { Editor } from '@tiptap/core';
  import { invoke } from '@tauri-apps/api/core';
  import EditorComponent from '$lib/editor/Editor.svelte';
  import Toolbar from '$lib/ui/Toolbar.svelte';
  import PageStack from '$lib/ui/PageStack.svelte';
  import StatusBar from '$lib/ui/StatusBar.svelte';
  import SettingsDialog from '$lib/ui/SettingsDialog.svelte';
  import AiSetupBanner from '$lib/ui/AiSetupBanner.svelte';
  import FileExplorer from '$lib/ui/FileExplorer.svelte';
  import { appState } from '$lib/state/app-state.svelte';
  import {
    openFile,
    openPath,
    saveFile,
    saveFileAs,
    newFile,
    promptUnsavedChanges,
    getSavedSnapshot,
    type EditorBridge,
  } from '$lib/state/file-ops';

  let editorComponent: EditorComponent;
  let toolbar: Toolbar;
  let editor = $state<Editor | undefined>(undefined);
  let contentHeight = $state(0);
  let pageBreaks = $state<number[]>([]);

  function handleContentHeightChange(height: number): void {
    contentHeight = height;
  }

  function handlePageBreaksChange(breaks: number[]): void {
    pageBreaks = breaks;
    appState.totalPages = breaks.length + 1;
  }

  /** Bridge between file-ops and the Tiptap editor. */
  function getBridge(): EditorBridge {
    return {
      getHTML: () => editorComponent?.getHTML() ?? '',
      setHTML: (html: string) => editorComponent?.setContent(html),
    };
  }

  /** Compute word and character counts from editor plain text. */
  function updateCounts(): void {
    const ed = editorComponent?.getEditor();
    if (!ed) return;
    const text = ed.getText();
    // Word count: split on whitespace, filter empty tokens
    const words = text.trim() === '' ? 0 : text.trim().split(/\s+/).length;
    // Char count without spaces
    const chars = text.replace(/\s/g, '').length;
    appState.wordCount = words;
    appState.charCount = chars;
  }

  function handleContentChange(): void {
    const current = editorComponent?.getHTML() ?? '';
    appState.isDirty = current !== getSavedSnapshot();
    updateCounts();
  }

  function handleSelectionUpdate(): void {
    // Bump tick on the toolbar to re-evaluate active states
    editor = editorComponent?.getEditor();
    toolbar?.bumpTick();
  }

  async function handleNew(): Promise<void> {
    await newFile(getBridge());
    updateCounts();
  }

  async function handleOpen(): Promise<void> {
    await openFile(getBridge());
    // After opening, update editor reference and counts
    editor = editorComponent?.getEditor();
    toolbar?.bumpTick();
    updateCounts();
  }

  async function handleOpenFromSidebar(path: string): Promise<void> {
    await openPath(path, getBridge());
    editor = editorComponent?.getEditor();
    toolbar?.bumpTick();
    updateCounts();
  }

  async function handleSave(): Promise<void> {
    await saveFile(getBridge());
  }

  function toggleAi(): void {
    if (!appState.aiAvailable) return;
    appState.aiEnabled = !appState.aiEnabled;
    editorComponent?.setGhostEnabled(appState.aiEnabled);
  }

  // Keep the ghost-completion plugin in sync with config + toggle state.
  // Without this, saving the API key in Settings flips aiAvailable but the
  // plugin stays disabled until the next app reload.
  $effect(() => {
    editorComponent?.setGhostEnabled(appState.aiEnabled && appState.aiAvailable);
  });

  onMount(() => {
    // Grab editor reference once mounted
    editor = editorComponent?.getEditor();

    const handleKeydown = (e: KeyboardEvent) => {
      // Cmd+N: New file
      if (e.metaKey && !e.shiftKey && e.key === 'n') {
        e.preventDefault();
        handleNew();
        return;
      }

      // Cmd+O: Open file
      if (e.metaKey && !e.shiftKey && e.key === 'o') {
        e.preventDefault();
        handleOpen();
        return;
      }

      // Cmd+S: Save
      if (e.metaKey && !e.shiftKey && e.key === 's') {
        e.preventDefault();
        handleSave();
        return;
      }

      // Cmd+Shift+S: Save As
      if (e.metaKey && e.shiftKey && e.key === 's') {
        e.preventDefault();
        saveFileAs(getBridge());
        return;
      }

      // Cmd+Shift+A: Toggle AI autocomplete
      if (e.metaKey && e.shiftKey && (e.key === 'a' || e.key === 'A')) {
        e.preventDefault();
        toggleAi();
        return;
      }

      // Cmd+,: Open settings
      if (e.metaKey && e.key === ',') {
        e.preventDefault();
        appState.settingsOpen = true;
        return;
      }

      // Cmd+B: Toggle file explorer sidebar
      if (e.metaKey && !e.shiftKey && (e.key === 'b' || e.key === 'B')) {
        e.preventDefault();
        appState.sidebarOpen = !appState.sidebarOpen;
        return;
      }
    };

    window.addEventListener('keydown', handleKeydown);

    // Query AI availability + persisted preferences from the backend on startup.
    (async () => {
      try {
        const status = await invoke<{
          enabled: boolean;
          active_provider: string;
          model: string;
        }>('get_ai_config');
        appState.setAiAvailable(status.enabled);
        appState.aiActiveProvider = status.active_provider;
      } catch {
        appState.setAiAvailable(false);
      }
      try {
        const meta = await invoke<{
          active_provider: string;
          providers: Record<string, {
            has_key: boolean;
            model: string;
            base_url: string;
            max_tokens: number;
          }>;
          trigger_speed: string;
        }>('load_ai_config');
        const activeSlot = meta.providers[meta.active_provider];
        if (activeSlot) {
          appState.aiMaxTokens = activeSlot.max_tokens;
        }
        if (meta.trigger_speed === 'eager' || meta.trigger_speed === 'balanced' || meta.trigger_speed === 'relaxed') {
          appState.aiTriggerSpeed = meta.trigger_speed;
        }
      } catch {
        // Store unavailable (dev browser); keep defaults.
      }
    })();

    // Register close handler for unsaved changes prompt
    let cleanupClose: (() => void) | undefined;

    (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const currentWindow = getCurrentWindow();
        const unlisten = await currentWindow.onCloseRequested(async (event) => {
          if (appState.isDirty) {
            event.preventDefault();

            const result = await promptUnsavedChanges();
            if (result === 'save') {
              await saveFile(getBridge());
              // If still dirty (save was cancelled), keep window open
              if (!appState.isDirty) {
                await currentWindow.destroy();
              }
            } else if (result === 'discard') {
              await currentWindow.destroy();
            }
            // 'cancel' does nothing, window stays open
          }
        });
        cleanupClose = unlisten;
      } catch {
        // Not in Tauri environment (dev browser)
      }
    })();

    return () => {
      window.removeEventListener('keydown', handleKeydown);
      cleanupClose?.();
    };
  });
</script>

<main class="app-root">
  <Toolbar
    bind:this={toolbar}
    {editor}
    onNew={handleNew}
    onOpen={handleOpen}
    onSave={handleSave}
    onSaveAs={() => saveFileAs(getBridge())}
    onToggleSidebar={() => (appState.sidebarOpen = !appState.sidebarOpen)}
    onToggleAi={toggleAi}
  />
  <div class="app-body">
    {#if appState.sidebarOpen}
      <FileExplorer onOpenFile={handleOpenFromSidebar} />
    {/if}
    <PageStack {contentHeight} {pageBreaks}>
      <EditorComponent
        bind:this={editorComponent}
        content=""
        onContentChange={handleContentChange}
        onSelectionUpdate={handleSelectionUpdate}
        onContentHeightChange={handleContentHeightChange}
        onPageBreaksChange={handlePageBreaksChange}
      />
    </PageStack>
  </div>
  <StatusBar />
  <SettingsDialog />
  <AiSetupBanner />
</main>

<style>
  .app-root {
    width: 100%;
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .app-body {
    flex: 1;
    display: flex;
    flex-direction: row;
    min-height: 0;
    overflow: hidden;
  }
</style>
