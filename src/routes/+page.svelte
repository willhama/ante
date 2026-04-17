<script lang="ts">
  import { onMount } from 'svelte';
  import type { Editor } from '@tiptap/core';
  import { invoke } from '@tauri-apps/api/core';
  import EditorComponent from '$lib/editor/Editor.svelte';
  import Toolbar from '$lib/ui/Toolbar.svelte';
  import PageStack from '$lib/ui/PageStack.svelte';
  import { appState } from '$lib/state/app-state.svelte';
  import {
    openFile,
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
  }

  /** Bridge between file-ops and the Tiptap editor. */
  function getBridge(): EditorBridge {
    return {
      getHTML: () => editorComponent?.getHTML() ?? '',
      setHTML: (html: string) => editorComponent?.setContent(html),
    };
  }

  function handleContentChange(): void {
    const current = editorComponent?.getHTML() ?? '';
    appState.isDirty = current !== getSavedSnapshot();
  }

  function handleSelectionUpdate(): void {
    // Bump tick on the toolbar to re-evaluate active states
    editor = editorComponent?.getEditor();
    toolbar?.bumpTick();
  }

  async function handleNew(): Promise<void> {
    await newFile(getBridge());
  }

  async function handleOpen(): Promise<void> {
    await openFile(getBridge());
    // After opening, update editor reference
    editor = editorComponent?.getEditor();
    toolbar?.bumpTick();
  }

  async function handleSave(): Promise<void> {
    await saveFile(getBridge());
  }

  function toggleAi(): void {
    if (!appState.aiAvailable) return;
    appState.aiEnabled = !appState.aiEnabled;
    editorComponent?.setGhostEnabled(appState.aiEnabled);
  }

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
    };

    window.addEventListener('keydown', handleKeydown);

    // Query AI availability from the backend on startup.
    (async () => {
      try {
        const status = await invoke<{ enabled: boolean; model: string }>('get_ai_config');
        appState.setAiAvailable(status.enabled);
        // Sync extension state with current toggle.
        editorComponent?.setGhostEnabled(appState.aiEnabled && status.enabled);
      } catch {
        appState.setAiAvailable(false);
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
    onToggleAi={toggleAi}
  />
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
</main>

<style>
  .app-root {
    width: 100%;
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
