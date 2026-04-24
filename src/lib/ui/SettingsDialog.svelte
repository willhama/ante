<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { appState, type AiTriggerSpeed, isAiTriggerSpeed } from '$lib/state/app-state.svelte';
  import { Button, buttonVariants } from '$lib/components/ui/button/index.js';
  import * as Dialog from '$lib/components/ui/dialog/index.js';
  import { Input } from '$lib/components/ui/input/index.js';
  import { Label } from '$lib/components/ui/label/index.js';
  import Sparkles from '@lucide/svelte/icons/sparkles';
  import ExternalLink from '@lucide/svelte/icons/external-link';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  type ProviderId = 'openai' | 'openai-compatible' | 'anthropic';

  interface ProviderMeta {
    has_key: boolean;
    model: string;
    base_url: string;
    max_tokens: number;
  }

  interface AiConfigMeta {
    active_provider: string;
    providers: Record<string, ProviderMeta>;
    trigger_speed: string;
  }

  interface AiTestResult {
    ok: boolean;
    model_count: number | null;
    error: string | null;
  }

  interface SaveProviderPayload {
    api_key?: string | null;
    model?: string | null;
    base_url?: string | null;
    max_tokens?: number | null;
  }

  interface SaveAiConfigPayload {
    active_provider?: string | null;
    trigger_speed?: string | null;
    providers?: Record<string, SaveProviderPayload>;
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

  const PROVIDER_LABELS: Record<ProviderId, string> = {
    openai: 'OpenAI',
    'openai-compatible': 'OpenAI-compatible',
    anthropic: 'Anthropic',
  };

  const PROVIDER_ORDER: ProviderId[] = ['openai', 'anthropic', 'openai-compatible'];

  const PROVIDER_DEFAULTS: Record<ProviderId, { model: string; base_url: string }> = {
    openai: { model: 'gpt-5.4-nano', base_url: 'https://api.openai.com/v1' },
    anthropic: { model: 'claude-haiku-4-5-20251001', base_url: 'https://api.anthropic.com' },
    'openai-compatible': { model: '', base_url: '' },
  };

  const KNOWN_MODELS_BY_PROVIDER: Record<
    ProviderId,
    { value: string; label: string; hint: string }[]
  > = {
    openai: [
      { value: 'gpt-5.4-nano', label: 'gpt-5.4-nano', hint: 'fast & cheap' },
      { value: 'gpt-5.4-mini', label: 'gpt-5.4-mini', hint: 'balanced' },
      { value: 'gpt-5.4', label: 'gpt-5.4', hint: 'flagship' },
      { value: 'gpt-5-mini', label: 'gpt-5-mini', hint: 'prev gen mini' },
      { value: 'gpt-5', label: 'gpt-5', hint: 'prev gen flagship' },
      { value: 'gpt-4.1-mini', label: 'gpt-4.1-mini', hint: 'legacy mini' },
      { value: 'gpt-4.1', label: 'gpt-4.1', hint: 'legacy' },
      { value: 'gpt-4o-mini', label: 'gpt-4o-mini', hint: 'legacy fast' },
    ],
    anthropic: [
      { value: 'claude-haiku-4-5-20251001', label: 'claude-haiku-4-5', hint: 'fast & cheap' },
      { value: 'claude-sonnet-4-6', label: 'claude-sonnet-4-6', hint: 'balanced quality' },
      { value: 'claude-opus-4-7', label: 'claude-opus-4-7', hint: 'top quality' },
    ],
    'openai-compatible': [],
  };

  const CUSTOM_MODEL_VALUE = '__custom__';

  /** Per-provider editable state. Mirrors the backend shape. */
  interface ProviderFormState {
    /** New API key typed by the user (never pre-filled). */
    apiKey: string;
    /** Whether the backend has a stored key for this provider. */
    hasKey: boolean;
    /** Selected model - either a known slug or CUSTOM_MODEL_VALUE. */
    modelSelect: string;
    /** Custom model input when modelSelect === CUSTOM_MODEL_VALUE. */
    model: string;
    /** Base URL (only shown for openai-compatible + under Advanced for openai/anthropic). */
    baseUrl: string;
    /** Max tokens (not shown per-provider; driven by suggestionLength below). */
    maxTokens: number;
    /** Debounced test-key state. */
    testResult: AiTestResult | null;
    testing: boolean;
    /** Whether the user modified this provider's fields this session (controls save payload). */
    dirty: boolean;
  }

  function emptyProviderForm(p: ProviderId): ProviderFormState {
    return {
      apiKey: '',
      hasKey: false,
      modelSelect: PROVIDER_DEFAULTS[p].model,
      model: PROVIDER_DEFAULTS[p].model,
      baseUrl: PROVIDER_DEFAULTS[p].base_url,
      maxTokens: 80,
      testResult: null,
      testing: false,
      dirty: false,
    };
  }

  let activeProvider = $state<ProviderId>('openai');
  let providerForms = $state<Record<ProviderId, ProviderFormState>>({
    openai: emptyProviderForm('openai'),
    'openai-compatible': emptyProviderForm('openai-compatible'),
    anthropic: emptyProviderForm('anthropic'),
  });

  let suggestionLength = $state<SuggestionLength>('medium');
  let triggerSpeed = $state<AiTriggerSpeed>('balanced');
  let showAdvanced = $state(false);
  let saving = $state(false);
  let saveError = $state('');
  let testTimer: number | null = null;

  const currentForm = $derived(providerForms[activeProvider]);
  const isCustomModel = $derived(currentForm.modelSelect === CUSTOM_MODEL_VALUE);
  const currentModelHint = $derived(
    KNOWN_MODELS_BY_PROVIDER[activeProvider].find((m) => m.value === currentForm.modelSelect)?.hint ?? '',
  );
  const isOpenAiCompatible = $derived(activeProvider === 'openai-compatible');

  $effect(() => {
    if (appState.settingsOpen) {
      loadConfig();
    } else {
      // Clear any pasted keys when dialog closes.
      for (const p of PROVIDER_ORDER) {
        providerForms[p].apiKey = '';
        providerForms[p].testResult = null;
        providerForms[p].testing = false;
        providerForms[p].dirty = false;
      }
      saveError = '';
      if (testTimer !== null) {
        window.clearTimeout(testTimer);
        testTimer = null;
      }
    }
  });

  async function runTest(provider: ProviderId, key: string): Promise<void> {
    providerForms[provider].testing = true;
    try {
      const baseUrl = providerForms[provider].baseUrl.trim();
      const result = await invoke<AiTestResult>('test_ai_config', {
        payload: {
          provider,
          api_key: key.trim() || null,
          base_url: baseUrl || null,
        },
      });
      providerForms[provider].testResult = result;
    } catch {
      providerForms[provider].testResult = {
        ok: false,
        model_count: null,
        error: 'test failed',
      };
    } finally {
      providerForms[provider].testing = false;
    }
  }

  /** Debounced auto-test when the user pastes/types a key. */
  function scheduleTest(): void {
    if (testTimer !== null) window.clearTimeout(testTimer);
    const form = providerForms[activeProvider];
    form.testResult = null;
    form.dirty = true;
    const key = form.apiKey.trim();
    if (key.length < 20) return;
    // For openai-compatible, require a base_url before testing - otherwise the
    // test will hit api.openai.com with the wrong key and the user will be
    // confused by a 401.
    if (activeProvider === 'openai-compatible' && !form.baseUrl.trim()) return;
    const provider = activeProvider;
    testTimer = window.setTimeout(() => {
      testTimer = null;
      runTest(provider, key);
    }, 400);
  }

  async function loadConfig(): Promise<void> {
    try {
      const meta = await invoke<AiConfigMeta>('load_ai_config');
      // Reset local state from backend.
      const backendActive = isProviderId(meta.active_provider) ? meta.active_provider : 'openai';
      activeProvider = backendActive;

      for (const p of PROVIDER_ORDER) {
        const slot = meta.providers[p] ?? {
          has_key: false,
          model: PROVIDER_DEFAULTS[p].model,
          base_url: PROVIDER_DEFAULTS[p].base_url,
          max_tokens: 80,
        };
        const effectiveModel = slot.model || PROVIDER_DEFAULTS[p].model;
        const knownModels = KNOWN_MODELS_BY_PROVIDER[p];
        const modelSelect =
          knownModels.some((m) => m.value === effectiveModel) && effectiveModel !== ''
            ? effectiveModel
            : CUSTOM_MODEL_VALUE;
        providerForms[p] = {
          apiKey: '',
          hasKey: slot.has_key,
          modelSelect,
          model: effectiveModel,
          baseUrl: slot.base_url || PROVIDER_DEFAULTS[p].base_url,
          maxTokens: slot.max_tokens || 80,
          testResult: null,
          testing: false,
          dirty: false,
        };
      }

      suggestionLength = tokensToLength(providerForms[activeProvider].maxTokens);
      triggerSpeed = isAiTriggerSpeed(meta.trigger_speed) ? meta.trigger_speed : 'balanced';
    } catch {
      // No store yet (dev browser).
    }
  }

  function isProviderId(v: string): v is ProviderId {
    return v === 'openai' || v === 'openai-compatible' || v === 'anthropic';
  }

  function onProviderChange(e: Event): void {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (isProviderId(v)) {
      activeProvider = v;
      suggestionLength = tokensToLength(providerForms[v].maxTokens);
    }
  }

  function onModelSelectChange(e: Event): void {
    const value = (e.currentTarget as HTMLSelectElement).value;
    const form = providerForms[activeProvider];
    form.modelSelect = value;
    form.dirty = true;
    if (value !== CUSTOM_MODEL_VALUE) form.model = value;
  }

  function onCustomModelInput(e: Event): void {
    providerForms[activeProvider].model = (e.currentTarget as HTMLInputElement).value;
    providerForms[activeProvider].dirty = true;
  }

  function onBaseUrlInput(e: Event): void {
    providerForms[activeProvider].baseUrl = (e.currentTarget as HTMLInputElement).value;
    providerForms[activeProvider].dirty = true;
  }

  async function handleSave(event?: Event): Promise<void> {
    event?.preventDefault();
    saving = true;
    saveError = '';
    try {
      // Build per-provider payload ONLY for providers the user touched,
      // plus the active provider's max_tokens (driven by suggestionLength).
      const providers: Record<string, SaveProviderPayload> = {};
      for (const p of PROVIDER_ORDER) {
        const form = providerForms[p];
        const touched = form.dirty || p === activeProvider;
        if (!touched) continue;
        const effectiveModel =
          (form.modelSelect === CUSTOM_MODEL_VALUE ? form.model : form.modelSelect).trim() ||
          PROVIDER_DEFAULTS[p].model;

        const providerPayload: SaveProviderPayload = {
          model: effectiveModel || null,
          base_url: form.baseUrl.trim() || null,
        };
        // api_key: only send if user typed something.
        //   empty string stays as null (untouched) - dedicated UI action would
        //   be needed to clear the stored key.
        if (form.apiKey.trim()) {
          providerPayload.api_key = form.apiKey.trim();
        }
        if (p === activeProvider) {
          providerPayload.max_tokens = LENGTH_TO_TOKENS[suggestionLength];
        } else {
          providerPayload.max_tokens = form.maxTokens;
        }
        providers[p] = providerPayload;
      }

      const payload: SaveAiConfigPayload = {
        active_provider: activeProvider,
        trigger_speed: triggerSpeed,
        providers,
      };

      await invoke('save_ai_config', { payload });
      const status = await invoke<{
        enabled: boolean;
        active_provider: string;
        model: string;
      }>('get_ai_config');
      appState.setAiAvailable(status.enabled);
      appState.aiActiveProvider = status.active_provider;
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
    const url =
      activeProvider === 'openai'
        ? 'https://platform.openai.com/api-keys'
        : activeProvider === 'anthropic'
          ? 'https://console.anthropic.com/settings/keys'
          : null;
    if (url) {
      openUrl(url).catch(() => {
        // Opener may be unavailable in dev browser; fall back silently.
      });
    }
  }

  const showKeyLink = $derived(activeProvider !== 'openai-compatible');

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
        {#if currentForm.hasKey}
          <span class="text-xs font-medium px-2 py-0.5 rounded-full bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-200">Key saved</span>
        {:else}
          <span class="text-xs font-medium px-2 py-0.5 rounded-full bg-muted text-muted-foreground">No key</span>
        {/if}
      </div>

      <div class="grid gap-3">
        <Label for="provider-select">Provider</Label>
        <select
          id="provider-select"
          class={selectClass}
          value={activeProvider}
          onchange={onProviderChange}
        >
          {#each PROVIDER_ORDER as p}
            <option value={p}>{PROVIDER_LABELS[p]}</option>
          {/each}
        </select>
        {#if isOpenAiCompatible}
          <p class="text-xs text-muted-foreground">
            Works with Groq, Together, OpenRouter, local Ollama (http://localhost:11434/v1),
            LM Studio, vLLM, and any endpoint that mimics OpenAI's /chat/completions API.
          </p>
        {/if}
      </div>

      <div class="grid gap-3">
        <Label for="api-key">API key</Label>
        <Input
          id="api-key"
          type="password"
          placeholder={currentForm.hasKey ? 'Enter new key to replace' : activeProvider === 'anthropic' ? 'sk-ant-...' : 'sk-...'}
          value={currentForm.apiKey}
          oninput={(e) => {
            providerForms[activeProvider].apiKey = (e.currentTarget as HTMLInputElement).value;
            scheduleTest();
          }}
          autocomplete="off"
          spellcheck={false}
        />
        {#if currentForm.testing}
          <p class="text-xs text-muted-foreground">Testing key{activeProvider === 'anthropic' ? ' (uses 1 token, fractions of a cent)' : ''}…</p>
        {:else if currentForm.testResult}
          {#if currentForm.testResult.ok}
            <p class="text-xs text-emerald-600 dark:text-emerald-400">
              Key valid{currentForm.testResult.model_count !== null ? ` - ${currentForm.testResult.model_count} models available` : ''}.
              {#if activeProvider === 'openai'}
                OpenAI does not expose credit balance to API keys; check platform.openai.com/usage.
              {/if}
            </p>
          {:else}
            <p class="text-xs text-destructive">{currentForm.testResult.error ?? 'Key check failed'}</p>
          {/if}
        {/if}
        <p class="text-xs text-muted-foreground">
          {currentForm.hasKey ? 'Leave blank to keep the existing key. Stored in your OS keychain.' : 'Required to enable AI ghost autocomplete. Stored in your OS keychain.'}
          {#if showKeyLink}
            <button
              type="button"
              onclick={openApiKeysPage}
              class="inline-flex items-center gap-1 text-primary underline underline-offset-2 hover:opacity-80"
            >
              Get your key<ExternalLink class="size-3" />
            </button>
          {/if}
        </p>
      </div>

      {#if isOpenAiCompatible}
        <div class="grid gap-3">
          <Label for="compat-base-url">Base URL</Label>
          <Input
            id="compat-base-url"
            type="text"
            placeholder="https://api.groq.com/openai/v1"
            value={currentForm.baseUrl}
            oninput={onBaseUrlInput}
            autocomplete="off"
            spellcheck={false}
          />
          <p class="text-xs text-muted-foreground">
            OpenAI-compatible /chat/completions endpoint. Required.
          </p>
        </div>

        <div class="grid gap-3">
          <Label for="compat-model">Model</Label>
          <Input
            id="compat-model"
            type="text"
            placeholder="model-id"
            value={currentForm.model}
            oninput={(e) => {
              providerForms[activeProvider].model = (e.currentTarget as HTMLInputElement).value;
              providerForms[activeProvider].modelSelect = CUSTOM_MODEL_VALUE;
              providerForms[activeProvider].dirty = true;
            }}
            autocomplete="off"
            spellcheck={false}
          />
        </div>
      {:else}
        <div class="grid gap-3">
          <Label for="model-select">Model</Label>
          <select
            id="model-select"
            class={selectClass}
            value={currentForm.modelSelect}
            onchange={onModelSelectChange}
          >
            {#each KNOWN_MODELS_BY_PROVIDER[activeProvider] as opt}
              <option value={opt.value}>{opt.label} - {opt.hint}</option>
            {/each}
            <option value={CUSTOM_MODEL_VALUE}>Custom…</option>
          </select>
          {#if isCustomModel}
            <Input
              id="model-custom"
              type="text"
              placeholder="model-id"
              value={currentForm.model}
              oninput={onCustomModelInput}
              autocomplete="off"
              spellcheck={false}
            />
          {:else if currentModelHint}
            <p class="text-xs text-muted-foreground">{currentModelHint}</p>
          {/if}
        </div>
      {/if}

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
            <option value="quick">Quick (500ms)</option>
            <option value="balanced">Balanced (800ms)</option>
          </select>
        </div>
      </div>
      <p class="text-xs text-muted-foreground -mt-2">
        Eager triggers sooner but spends more API calls while you think.
      </p>

      {#if !isOpenAiCompatible}
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
              placeholder={PROVIDER_DEFAULTS[activeProvider].base_url}
              value={currentForm.baseUrl}
              oninput={onBaseUrlInput}
              autocomplete="off"
              spellcheck={false}
            />
            <p class="text-xs text-muted-foreground">
              {activeProvider === 'anthropic'
                ? 'Anthropic API endpoint (rarely needs overriding).'
                : 'OpenAI API endpoint (rarely needs overriding).'}
            </p>
          </div>
        {/if}
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
