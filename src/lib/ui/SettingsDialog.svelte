<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { appState, type AiTriggerSpeed } from '$lib/state/app-state.svelte';
  import { Button, buttonVariants } from '$lib/components/ui/button/index.js';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Label } from '$lib/components/ui/label/index.js';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  interface AiConfigMeta {
    has_key: boolean;
    model: string;
    base_url: string;
    max_tokens: number;
    trigger_speed: string;
  }

  interface AiTestResult {
    ok: boolean;
    model_count: number | null;
    error: string | null;
  }

  type SuggestionLength = 'short' | 'medium' | 'long';

  const LENGTH_TO_TOKENS: Record<SuggestionLength, number> = {
    short: 40,
    medium: 80,
    long: 160,
  };

  function tokensToLength(tokens: number): SuggestionLength {
    if (tokens <= 50) return 'short';
    if (tokens <= 110) return 'medium';
    return 'long';
  }

  const KNOWN_MODELS: { value: string; label: string; hint: string }[] = [
    { value: 'gpt-4o-mini', label: 'gpt-4o-mini', hint: 'fast & cheap' },
    { value: 'gpt-4o', label: 'gpt-4o', hint: 'balanced quality' },
    { value: 'gpt-4.1-mini', label: 'gpt-4.1-mini', hint: 'newer mini' },
    { value: 'gpt-4.1', label: 'gpt-4.1', hint: 'flagship quality' },
    { value: 'gpt-3.5-turbo', label: 'gpt-3.5-turbo', hint: 'legacy, cheapest' },
  ];
  const CUSTOM_MODEL_VALUE = '__custom__';

  let apiKey = $state('');
  let model = $state('gpt-4o-mini');
  let modelSelect = $state('gpt-4o-mini');
  let baseUrl = $state('https://api.openai.com/v1');
  let suggestionLength = $state<SuggestionLength>('medium');
  let triggerSpeed = $state<AiTriggerSpeed>('balanced');
  let showAdvanced = $state(false);
  let saving = $state(false);
  let saveError = $state('');
  let hasKey = $state(false);
  let testing = $state(false);
  let testResult = $state<AiTestResult | null>(null);
  let testTimer: number | null = null;

  const isCustomModel = $derived(modelSelect === CUSTOM_MODEL_VALUE);
  const currentModelHint = $derived(
    KNOWN_MODELS.find((m) => m.value === modelSelect)?.hint ?? '',
  );

  $effect(() => {
    if (appState.settingsOpen) {
      loadConfig();
    } else {
      apiKey = '';
      saveError = '';
      testResult = null;
      if (testTimer !== null) {
        window.clearTimeout(testTimer);
        testTimer = null;
      }
    }
  });

  async function runTest(key: string): Promise<void> {
    testing = true;
    try {
      testResult = await invoke<AiTestResult>('test_ai_config', {
        payload: {
          api_key: key.trim() || null,
          base_url: baseUrl.trim() || null,
        },
      });
    } catch {
      testResult = { ok: false, model_count: null, error: 'test failed' };
    } finally {
      testing = false;
    }
  }

  /** Debounced auto-test when the user pastes/types a key. */
  function scheduleTest(): void {
    if (testTimer !== null) window.clearTimeout(testTimer);
    testResult = null;
    const key = apiKey.trim();
    if (key.length < 20) return; // ignore partial input
    testTimer = window.setTimeout(() => {
      testTimer = null;
      runTest(key);
    }, 400);
  }

  async function loadConfig(): Promise<void> {
    try {
      const meta = await invoke<AiConfigMeta>('load_ai_config');
      hasKey = meta.has_key;
      model = meta.model || 'gpt-4o-mini';
      baseUrl = meta.base_url || 'https://api.openai.com/v1';
      suggestionLength = tokensToLength(meta.max_tokens || 80);
      triggerSpeed =
        meta.trigger_speed === 'eager' ||
        meta.trigger_speed === 'balanced' ||
        meta.trigger_speed === 'relaxed'
          ? meta.trigger_speed
          : 'balanced';
      modelSelect = KNOWN_MODELS.some((m) => m.value === model) ? model : CUSTOM_MODEL_VALUE;
      apiKey = '';
    } catch {
      // No store yet (dev browser).
    }
  }

  function onModelSelectChange(e: Event): void {
    const value = (e.currentTarget as HTMLSelectElement).value;
    modelSelect = value;
    if (value !== CUSTOM_MODEL_VALUE) model = value;
  }

  async function handleSave(event?: Event): Promise<void> {
    event?.preventDefault();
    saving = true;
    saveError = '';
    try {
      const effectiveModel = (isCustomModel ? model : modelSelect).trim() || 'gpt-4o-mini';
      await invoke('save_ai_config', {
        payload: {
          api_key: apiKey.trim() || null,
          model: effectiveModel,
          base_url: baseUrl.trim() || null,
          max_tokens: LENGTH_TO_TOKENS[suggestionLength],
          trigger_speed: triggerSpeed,
        },
      });
      const status = await invoke<{ enabled: boolean; model: string }>('get_ai_config');
      appState.setAiAvailable(status.enabled);
      appState.aiMaxTokens = LENGTH_TO_TOKENS[suggestionLength];
      appState.aiTriggerSpeed = triggerSpeed;
      appState.settingsOpen = false;
    } catch {
      saveError = 'Failed to save settings. Please try again.';
    } finally {
      saving = false;
    }
  }

  function openApiKeysPage(): void {
    window.open('https://platform.openai.com/api-keys', '_blank', 'noopener,noreferrer');
  }

  const selectClass =
    'flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-xs transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50';
</script>

<Dialog.Root bind:open={appState.settingsOpen}>
  <Dialog.Content class="sm:max-w-[520px]">
    <Dialog.Header>
      <Dialog.Title>Settings</Dialog.Title>
      <Dialog.Description>Configure ante preferences and integrations.</Dialog.Description>
    </Dialog.Header>

    <form onsubmit={handleSave} class="grid gap-4">
      <div class="flex items-center gap-2 pb-3 border-b">
        <Sparkles class="size-4 text-muted-foreground" />
        <h3 class="text-sm font-semibold flex-1">AI Autocomplete</h3>
        {#if hasKey}
          <span class="text-xs font-medium px-2 py-0.5 rounded-full bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200">Key saved</span>
        {:else}
          <span class="text-xs font-medium px-2 py-0.5 rounded-full bg-muted text-muted-foreground">No key</span>
        {/if}
      </div>

      <div class="grid gap-3">
        <Label for="api-key">OpenAI API key</Label>
        <Input
          id="api-key"
          type="password"
          placeholder={hasKey ? 'Enter new key to replace' : 'sk-...'}
          bind:value={apiKey}
          oninput={scheduleTest}
          autocomplete="off"
          spellcheck={false}
        />
        {#if testing}
          <p class="text-xs text-muted-foreground">Testing key…</p>
        {:else if testResult}
          {#if testResult.ok}
            <p class="text-xs text-emerald-600 dark:text-emerald-400">
              Key valid{testResult.model_count !== null ? ` — ${testResult.model_count} models available` : ''}.
              OpenAI does not expose credit balance to API keys; check platform.openai.com/usage.
            </p>
          {:else}
            <p class="text-xs text-destructive">{testResult.error ?? 'Key check failed'}</p>
          {/if}
        {/if}
        <p class="text-xs text-muted-foreground">
          {hasKey ? 'Leave blank to keep the existing key.' : 'Required to enable AI ghost autocomplete.'}
          <button
            type="button"
            onclick={openApiKeysPage}
            class="inline-flex items-center gap-1 text-primary underline underline-offset-2 hover:opacity-80"
          >
            Get your key<ExternalLink class="size-3" />
          </button>
        </p>
      </div>

      <div class="grid gap-3">
        <Label for="model-select">Model</Label>
        <select
          id="model-select"
          class={selectClass}
          value={modelSelect}
          onchange={onModelSelectChange}
        >
          {#each KNOWN_MODELS as opt}
            <option value={opt.value}>{opt.label} — {opt.hint}</option>
          {/each}
          <option value={CUSTOM_MODEL_VALUE}>Custom…</option>
        </select>
        {#if isCustomModel}
          <Input
            id="model-custom"
            type="text"
            placeholder="model-id"
            bind:value={model}
            autocomplete="off"
            spellcheck={false}
          />
        {:else if currentModelHint}
          <p class="text-xs text-muted-foreground">{currentModelHint}</p>
        {/if}
      </div>

      <div class="grid grid-cols-2 gap-3">
        <div class="grid gap-3">
          <Label for="suggestion-length">Suggestion length</Label>
          <select id="suggestion-length" class={selectClass} bind:value={suggestionLength}>
            <option value="short">Short (~40 tokens)</option>
            <option value="medium">Medium (~80 tokens)</option>
            <option value="long">Long (~160 tokens)</option>
          </select>
        </div>

        <div class="grid gap-3">
          <Label for="trigger-speed">Trigger speed</Label>
          <select id="trigger-speed" class={selectClass} bind:value={triggerSpeed}>
            <option value="eager">Eager (300ms)</option>
            <option value="balanced">Balanced (800ms)</option>
            <option value="relaxed">Relaxed (1.5s)</option>
          </select>
        </div>
      </div>
      <p class="text-xs text-muted-foreground -mt-2">
        Eager triggers sooner but spends more API calls while you think.
      </p>

      <button
        type="button"
        class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground self-start"
        onclick={() => (showAdvanced = !showAdvanced)}
      >
        <ChevronRight class="size-3 transition-transform {showAdvanced ? 'rotate-90' : ''}" />
        Advanced
      </button>

      {#if showAdvanced}
        <div class="grid gap-3">
          <Label for="base-url">Base URL</Label>
          <Input
            id="base-url"
            type="text"
            placeholder="https://api.openai.com/v1"
            bind:value={baseUrl}
            autocomplete="off"
            spellcheck={false}
          />
          <p class="text-xs text-muted-foreground">OpenAI-compatible API endpoint.</p>
        </div>
      {/if}

      {#if saveError}
        <p class="text-sm text-destructive">{saveError}</p>
      {/if}

      <Dialog.Footer>
        <Dialog.Close type="button" class={buttonVariants({ variant: 'outline' })}>
          Cancel
        </Dialog.Close>
        <Button type="submit" disabled={saving}>
          {saving ? 'Saving...' : 'Save changes'}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
