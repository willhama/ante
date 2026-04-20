//! Multi-provider AI autocomplete wiring.
//!
//! This module owns:
//! - Config shape (per-provider settings + active provider + trigger_speed)
//! - Legacy v0 -> v1 migration (single OpenAI key in JSON -> providers.openai.api_key)
//! - Tauri commands: stream_completion / cancel_completion / get_ai_config /
//!   load_ai_config / save_ai_config / test_ai_config
//!
//! Provider-specific wire format lives in submodules:
//! - `provider`: `ProviderClient` trait, shared types
//! - `openai`:   OpenAI + OpenAI-compatible impl
//! - `anthropic`: Anthropic `/v1/messages` impl
//!
//! API keys are stored as plain strings in `ai-config.json` under
//! `providers.<slug>.api_key`. They are never returned to the frontend -
//! `load_ai_config` returns only `has_key: bool` per provider.

pub mod anthropic;
pub mod openai;
pub mod provider;

use crate::errors::AnteError;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use tokio::sync::oneshot;

pub use provider::{
    extract_provider_error, AiTestResult, ParseOutcome, Provider, ProviderClient, StreamParams,
};

use crate::ai::anthropic::{AnthropicClient, ANTHROPIC_DEFAULT_BASE_URL, ANTHROPIC_DEFAULT_MODEL};
use crate::ai::openai::{OpenAiClient, OpenAiCompatibleClient, OPENAI_DEFAULT_BASE_URL, OPENAI_DEFAULT_MODEL};

const STORE_FILE: &str = "ai-config.json";
const KEY_ACTIVE_PROVIDER: &str = "active_provider";
const KEY_PROVIDERS: &str = "providers";
const KEY_TRIGGER_SPEED: &str = "trigger_speed";
const KEY_MIGRATED: &str = "_migrated_v1";

// Legacy (pre-v1) store keys - used only for migration.
const LEGACY_KEY_API_KEY: &str = "api_key";
const LEGACY_KEY_MODEL: &str = "model";
const LEGACY_KEY_BASE_URL: &str = "base_url";
const LEGACY_KEY_MAX_TOKENS: &str = "max_tokens";

const SYSTEM_PROMPT_BASE: &str = "You complete the user's text. Output only the continuation, no explanations, no quotes, no markdown. Match the user's style and tone.";

/// Map max_tokens to a length hint the model actually respects.
fn length_hint(max_tokens: u32) -> &'static str {
    match max_tokens {
        0..=50 => "Continue with a single short sentence or phrase.",
        51..=110 => "Continue naturally for 1-2 sentences.",
        _ => "Continue naturally for 3-5 sentences, forming a short paragraph.",
    }
}

const DEFAULT_MAX_TOKENS: u32 = 80;
const DEFAULT_TRIGGER_SPEED: &str = "balanced";
const TEMPERATURE: f32 = 0.3;

fn is_valid_trigger_speed(s: &str) -> bool {
    matches!(s, "eager" | "quick" | "balanced")
}

/// Returns a boxed provider client for the given provider.
pub fn client_for(provider: Provider) -> Box<dyn ProviderClient> {
    match provider {
        Provider::Openai => Box::new(OpenAiClient),
        Provider::OpenaiCompatible => Box::new(OpenAiCompatibleClient),
        Provider::Anthropic => Box::new(AnthropicClient),
    }
}

/// Fully resolved per-provider settings, suitable for dispatching a stream.
#[derive(Clone, Debug)]
struct ProviderConfig {
    provider: Provider,
    api_key: Option<String>,
    base_url: String,
    model: String,
    max_tokens: u32,
}

/// Top-level config snapshot (active provider + resolved settings + trigger_speed).
#[derive(Clone, Debug)]
struct ResolvedConfig {
    active: ProviderConfig,
    #[allow(dead_code)] // Consumed via a separate store read in load_ai_config.
    trigger_speed: String,
}

/// Non-secret per-provider metadata surfaced to the UI by `load_ai_config`.
#[derive(Serialize, Debug)]
pub struct ProviderMeta {
    pub has_key: bool,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
}

/// Status payload returned to the frontend. Never includes any API key.
#[derive(Serialize, Debug)]
pub struct AiStatus {
    pub enabled: bool,
    pub active_provider: String,
    pub model: String,
}

/// Full config metadata for the settings UI.
#[derive(Serialize, Debug)]
pub struct AiConfigMeta {
    pub active_provider: String,
    pub providers: HashMap<String, ProviderMeta>,
    pub trigger_speed: String,
}

/// Per-provider save payload. api_key tri-state:
///   None         -> untouched (leave existing value as-is)
///   Some("")     -> clear to empty string in JSON
///   Some("sk-…") -> write new value
#[derive(Deserialize, Debug)]
pub struct SaveProviderPayload {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct SaveAiConfigPayload {
    #[serde(default)]
    pub active_provider: Option<String>,
    #[serde(default)]
    pub trigger_speed: Option<String>,
    #[serde(default)]
    pub providers: Option<HashMap<String, SaveProviderPayload>>,
}

/// Test payload - optional provider (defaults to active) plus optional
/// api_key/base_url overrides (test a just-pasted key before saving it).
#[derive(Deserialize, Debug)]
pub struct TestAiConfigPayload {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
}

/// Tracks cancellation senders for in-flight completion streams.
#[derive(Default)]
pub struct AiState {
    cancellers: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

fn default_model_for(p: Provider) -> &'static str {
    match p {
        Provider::Openai => OPENAI_DEFAULT_MODEL,
        Provider::OpenaiCompatible => "",
        Provider::Anthropic => ANTHROPIC_DEFAULT_MODEL,
    }
}

fn default_base_url_for(p: Provider) -> &'static str {
    match p {
        Provider::Openai => OPENAI_DEFAULT_BASE_URL,
        Provider::OpenaiCompatible => "",
        Provider::Anthropic => ANTHROPIC_DEFAULT_BASE_URL,
    }
}

fn env_var_for(p: Provider) -> &'static str {
    match p {
        Provider::Openai => "ANTE_OPENAI_API_KEY",
        Provider::OpenaiCompatible => "ANTE_OPENAI_COMPATIBLE_API_KEY",
        Provider::Anthropic => "ANTE_ANTHROPIC_API_KEY",
    }
}

/// One-time migration from the v0 (single-provider OpenAI) JSON shape to v1.
///
/// v0 had top-level keys `api_key`, `model`, `base_url`, `max_tokens`.
/// v1 nests them under `providers.openai.{api_key,model,base_url,max_tokens}`.
///
/// The legacy api_key is moved into `providers.openai.api_key` in the JSON
/// store - no keychain involved. Guarded by `_migrated_v1: true`; idempotent.
fn run_migration_if_needed(app: &AppHandle) {
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };

    let already_migrated = store
        .get(KEY_MIGRATED)
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let has_legacy_api_key = store.get(LEGACY_KEY_API_KEY).is_some();
    let has_legacy_model = store.get(LEGACY_KEY_MODEL).is_some();
    let has_legacy_base_url = store.get(LEGACY_KEY_BASE_URL).is_some();
    let has_legacy_max_tokens = store.get(LEGACY_KEY_MAX_TOKENS).is_some();

    // Fast path - nothing to do.
    if already_migrated
        && !has_legacy_api_key
        && !has_legacy_model
        && !has_legacy_base_url
        && !has_legacy_max_tokens
    {
        return;
    }

    // Read legacy values.
    let legacy_api_key = store
        .get(LEGACY_KEY_API_KEY)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty());
    let legacy_model = store
        .get(LEGACY_KEY_MODEL)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty());
    let legacy_base_url = store
        .get(LEGACY_KEY_BASE_URL)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty());
    let legacy_max_tokens = store
        .get(LEGACY_KEY_MAX_TOKENS)
        .and_then(|v| v.as_u64())
        .map(|n| n.clamp(1, 4096) as u32);

    // Build/merge the providers map.
    let mut providers: serde_json::Map<String, serde_json::Value> = store
        .get(KEY_PROVIDERS)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    for p in Provider::all() {
        let slug = p.as_str();
        let mut entry = providers
            .get(slug)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        // OpenAI slot picks up the legacy values.
        if p == Provider::Openai {
            if entry.get("api_key").is_none() {
                if let Some(ref k) = legacy_api_key {
                    entry.insert("api_key".to_string(), serde_json::Value::String(k.clone()));
                }
            }
            if entry.get("model").is_none() {
                if let Some(ref m) = legacy_model {
                    entry.insert("model".to_string(), serde_json::Value::String(m.clone()));
                }
            }
            if entry.get("base_url").is_none() {
                if let Some(ref b) = legacy_base_url {
                    entry.insert("base_url".to_string(), serde_json::Value::String(b.clone()));
                }
            }
            if entry.get("max_tokens").is_none() {
                if let Some(mt) = legacy_max_tokens {
                    entry.insert("max_tokens".to_string(), serde_json::Value::from(mt));
                }
            }
        }

        // Fill defaults for any missing fields.
        entry
            .entry("api_key".to_string())
            .or_insert_with(|| serde_json::Value::String(String::new()));
        entry
            .entry("model".to_string())
            .or_insert_with(|| serde_json::Value::String(default_model_for(p).to_string()));
        entry
            .entry("base_url".to_string())
            .or_insert_with(|| serde_json::Value::Null);
        entry
            .entry("max_tokens".to_string())
            .or_insert_with(|| serde_json::Value::from(DEFAULT_MAX_TOKENS));

        providers.insert(slug.to_string(), serde_json::Value::Object(entry));
    }

    store.set(KEY_PROVIDERS, serde_json::Value::Object(providers));

    if store.get(KEY_ACTIVE_PROVIDER).is_none() {
        store.set(
            KEY_ACTIVE_PROVIDER,
            serde_json::Value::String(Provider::Openai.as_str().to_string()),
        );
    }

    // Drop legacy top-level fields.
    store.delete(LEGACY_KEY_API_KEY);
    store.delete(LEGACY_KEY_MODEL);
    store.delete(LEGACY_KEY_BASE_URL);
    store.delete(LEGACY_KEY_MAX_TOKENS);
    store.set(KEY_MIGRATED, serde_json::Value::Bool(true));

    // One-time deprecation warning for legacy env vars.
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED_LEGACY_ENV: AtomicBool = AtomicBool::new(false);
    if !WARNED_LEGACY_ENV.swap(true, Ordering::AcqRel) {
        let has_legacy_env = std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("OPENAI_BASE_URL").is_ok()
            || std::env::var("OPENAI_MODEL").is_ok();
        if has_legacy_env {
            eprintln!(
                "ai: OPENAI_* env vars are deprecated; use ANTE_OPENAI_API_KEY / \
                 ANTE_OPENAI_COMPATIBLE_API_KEY / ANTE_ANTHROPIC_API_KEY instead. \
                 OPENAI_API_KEY is still honored as a fallback for the openai provider in v1."
            );
        }
    }

    let _ = store.save();
}

/// Read one provider's non-key fields from the JSON store.
/// Returns (model, base_url, max_tokens) with defaults filled in.
fn read_provider_slot(
    store: &tauri_plugin_store::Store<tauri::Wry>,
    provider: Provider,
) -> (String, String, u32) {
    let providers = store
        .get(KEY_PROVIDERS)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let slot = providers
        .get(provider.as_str())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();

    let model = slot
        .get("model")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_model_for(provider).to_string());

    let base_url = slot
        .get("base_url")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_base_url_for(provider).to_string());

    let max_tokens = slot
        .get("max_tokens")
        .and_then(|v| v.as_u64())
        .map(|n| n.clamp(1, 4096) as u32)
        .unwrap_or(DEFAULT_MAX_TOKENS);

    (model, base_url, max_tokens)
}

/// Read a provider's api_key from the JSON store. Returns None if absent or empty.
fn read_provider_key(
    store: &tauri_plugin_store::Store<tauri::Wry>,
    provider: Provider,
) -> Option<String> {
    store
        .get(KEY_PROVIDERS)
        .and_then(|v| v.as_object().cloned())
        .and_then(|providers| {
            providers
                .get(provider.as_str())
                .and_then(|slot| slot.as_object().cloned())
        })
        .and_then(|slot| {
            slot.get("api_key")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
        })
        .filter(|s| !s.trim().is_empty())
}

/// Resolve full config from store + env-var fallback.
fn get_config(app: &AppHandle) -> ResolvedConfig {
    run_migration_if_needed(app);

    let (active_provider, trigger_speed, model, base_url, max_tokens, store_key) =
        if let Ok(store) = app.store(STORE_FILE) {
            let active_str = store
                .get(KEY_ACTIVE_PROVIDER)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| Provider::Openai.as_str().to_string());
            let active = Provider::from_str(&active_str).unwrap_or(Provider::Openai);

            let trigger_speed = store
                .get(KEY_TRIGGER_SPEED)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .filter(|s| is_valid_trigger_speed(s.as_str()))
                .unwrap_or_else(|| DEFAULT_TRIGGER_SPEED.to_string());

            let (model, base_url, max_tokens) = read_provider_slot(&store, active);
            let store_key = read_provider_key(&store, active);
            (active, trigger_speed, model, base_url, max_tokens, store_key)
        } else {
            // Store unavailable (e.g. vite dev without Tauri context).
            (
                Provider::Openai,
                DEFAULT_TRIGGER_SPEED.to_string(),
                default_model_for(Provider::Openai).to_string(),
                default_base_url_for(Provider::Openai).to_string(),
                DEFAULT_MAX_TOKENS,
                None,
            )
        };

    // Resolve API key: JSON store → per-provider env var → legacy OPENAI_API_KEY
    // (openai provider only, v1 compatibility fallback).
    let api_key = store_key
        .or_else(|| {
            std::env::var(env_var_for(active_provider))
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            if active_provider == Provider::Openai {
                std::env::var("OPENAI_API_KEY")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            } else {
                None
            }
        });

    ResolvedConfig {
        active: ProviderConfig {
            provider: active_provider,
            api_key,
            base_url,
            model,
            max_tokens,
        },
        trigger_speed,
    }
}

/// Sanitize error messages so we never leak API keys, URLs, or headers to the UI.
pub(crate) fn sanitize_error(raw: impl std::fmt::Display) -> String {
    let s = raw.to_string();
    if s.to_lowercase().contains("timeout") {
        "request timed out".to_string()
    } else if s.to_lowercase().contains("connect") {
        "connection failed".to_string()
    } else {
        "request failed".to_string()
    }
}

#[derive(Serialize, Clone)]
struct ChunkPayload {
    id: String,
    text: String,
}

#[derive(Serialize, Clone)]
struct DonePayload {
    id: String,
}

#[derive(Serialize, Clone)]
struct ErrorPayload {
    id: String,
    message: String,
}

/// Starts a streaming completion. Returns a request_id for cancellation.
#[tauri::command]
pub async fn stream_completion(
    app: AppHandle,
    state: State<'_, AiState>,
    context_before: String,
    context_after: String,
) -> Result<String, AnteError> {
    let config = get_config(&app);
    let api_key = match config.active.api_key.clone() {
        Some(k) => k,
        None => return Err(AnteError::ApiError("not configured".to_string())),
    };

    if config.active.provider == Provider::OpenaiCompatible
        && config.active.base_url.trim().is_empty()
    {
        return Err(AnteError::ApiError("no base_url configured".to_string()));
    }

    let request_id = uuid::Uuid::new_v4().to_string();
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

    {
        let mut map = state.cancellers.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(request_id.clone(), cancel_tx);
    }

    let app_clone = app.clone();
    let req_id = request_id.clone();
    let provider = config.active.provider;
    let base_url = config.active.base_url.clone();
    let model = config.active.model.clone();
    let max_tokens = config.active.max_tokens;

    tokio::spawn(async move {
        let client = client_for(provider);
        run_stream(
            app_clone.clone(),
            req_id.clone(),
            client,
            api_key,
            base_url,
            model,
            max_tokens,
            context_before,
            context_after,
            cancel_rx,
        )
        .await;

        if let Some(ai_state) = app_clone.try_state::<AiState>() {
            let mut map = ai_state.cancellers.lock().unwrap_or_else(|e| e.into_inner());
            map.remove(&req_id);
        }
    });

    Ok(request_id)
}

/// The main streaming loop. Provider-agnostic: delegates to `ProviderClient`.
#[allow(clippy::too_many_arguments)]
async fn run_stream(
    app: AppHandle,
    id: String,
    provider_client: Box<dyn ProviderClient>,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: u32,
    context_before: String,
    context_after: String,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    let user_content = format!("{}{{CURSOR}}{}", context_before, context_after);
    let system_prompt = format!("{} {}", SYSTEM_PROMPT_BASE, length_hint(max_tokens));

    let params = StreamParams {
        api_key: &api_key,
        base_url: &base_url,
        model: &model,
        max_tokens,
        system: &system_prompt,
        user_content: &user_content,
        temperature: TEMPERATURE,
    };

    let http = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            emit_error(&app, &id, sanitize_error(e));
            return;
        }
    };

    let request_fut = provider_client.build_request(&http, &params).send();

    let response = tokio::select! {
        biased;
        _ = &mut cancel_rx => return,
        r = request_fut => r,
    };

    let response = match response {
        Ok(r) => r,
        Err(e) => {
            emit_error(&app, &id, sanitize_error(e));
            return;
        }
    };

    if !response.status().is_success() {
        let code = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        let msg = extract_provider_error(&body).unwrap_or_else(|| match code {
            401 => "auth failed".to_string(),
            429 => "rate limited".to_string(),
            _ => format!("http {code}"),
        });
        emit_error(&app, &id, msg);
        return;
    }

    let mut stream = response.bytes_stream().eventsource();

    loop {
        tokio::select! {
            biased;
            _ = &mut cancel_rx => return,
            next = stream.next() => {
                match next {
                    None => break,
                    Some(Err(e)) => {
                        emit_error(&app, &id, sanitize_error(e));
                        return;
                    }
                    Some(Ok(event)) => {
                        let data = event.data.trim();
                        if data.is_empty() {
                            continue;
                        }
                        match provider_client.parse_sse_data(&event.event, data) {
                            ParseOutcome::Token(text) => {
                                let _ = app.emit(
                                    "completion-chunk",
                                    ChunkPayload { id: id.clone(), text },
                                );
                            }
                            ParseOutcome::Done => {
                                let _ = app.emit(
                                    "completion-done",
                                    DonePayload { id: id.clone() },
                                );
                                return;
                            }
                            ParseOutcome::Ignore => continue,
                        }
                    }
                }
            }
        }
    }

    let _ = app.emit("completion-done", DonePayload { id: id.clone() });
}

fn emit_error(app: &AppHandle, id: &str, message: String) {
    let _ = app.emit(
        "completion-error",
        ErrorPayload {
            id: id.to_string(),
            message,
        },
    );
}

/// Cancels an in-flight completion stream.
#[tauri::command]
pub async fn cancel_completion(
    state: State<'_, AiState>,
    request_id: String,
) -> Result<(), AnteError> {
    let tx_opt = {
        let mut map = state.cancellers.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&request_id)
    };
    if let Some(tx) = tx_opt {
        let _ = tx.send(());
    }
    Ok(())
}

/// Returns whether the active provider has a key (AI enabled) plus provider
/// slug and model.
#[tauri::command]
pub fn get_ai_config(app: AppHandle) -> AiStatus {
    let config = get_config(&app);
    AiStatus {
        enabled: config.active.api_key.is_some(),
        active_provider: config.active.provider.as_str().to_string(),
        model: config.active.model,
    }
}

/// Returns full config metadata for the settings UI.
/// `has_key` is true iff the stored api_key string is non-empty.
/// The key value itself is never returned.
#[tauri::command]
pub fn load_ai_config(app: AppHandle) -> AiConfigMeta {
    run_migration_if_needed(&app);

    let active_provider = if let Ok(store) = app.store(STORE_FILE) {
        store
            .get(KEY_ACTIVE_PROVIDER)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .and_then(|s| Provider::from_str(&s).map(|p| p.as_str().to_string()))
            .unwrap_or_else(|| Provider::Openai.as_str().to_string())
    } else {
        Provider::Openai.as_str().to_string()
    };

    let trigger_speed = if let Ok(store) = app.store(STORE_FILE) {
        store
            .get(KEY_TRIGGER_SPEED)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| is_valid_trigger_speed(s.as_str()))
            .unwrap_or_else(|| DEFAULT_TRIGGER_SPEED.to_string())
    } else {
        DEFAULT_TRIGGER_SPEED.to_string()
    };

    let mut providers_map: HashMap<String, ProviderMeta> = HashMap::new();
    if let Ok(store) = app.store(STORE_FILE) {
        for p in Provider::all() {
            let (model, base_url, max_tokens) = read_provider_slot(&store, p);
            let has_key = read_provider_key(&store, p).is_some();
            providers_map.insert(
                p.as_str().to_string(),
                ProviderMeta { has_key, model, base_url, max_tokens },
            );
        }
    } else {
        for p in Provider::all() {
            providers_map.insert(
                p.as_str().to_string(),
                ProviderMeta {
                    has_key: false,
                    model: default_model_for(p).to_string(),
                    base_url: default_base_url_for(p).to_string(),
                    max_tokens: DEFAULT_MAX_TOKENS,
                },
            );
        }
    }

    AiConfigMeta { active_provider, providers: providers_map, trigger_speed }
}

/// Persists AI configuration to the JSON store.
///
/// api_key tri-state per provider:
///   None         -> leave existing value untouched
///   Some("")     -> clear to empty string (disables that provider)
///   Some("sk-…") -> write new value
#[tauri::command]
pub fn save_ai_config(
    app: AppHandle,
    payload: SaveAiConfigPayload,
) -> Result<(), AnteError> {
    run_migration_if_needed(&app);

    let store = app
        .store(STORE_FILE)
        .map_err(|e| AnteError::ApiError(format!("store error: {e}")))?;

    if let Some(active) = payload.active_provider.as_deref() {
        if Provider::from_str(active).is_some() {
            store.set(KEY_ACTIVE_PROVIDER, serde_json::Value::String(active.to_string()));
        }
    }

    if let Some(speed) = payload.trigger_speed.as_deref() {
        if is_valid_trigger_speed(speed) {
            store.set(KEY_TRIGGER_SPEED, serde_json::Value::String(speed.to_string()));
        }
    }

    if let Some(providers_payload) = payload.providers {
        let mut providers_map: serde_json::Map<String, serde_json::Value> = store
            .get(KEY_PROVIDERS)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();

        for (slug, p) in providers_payload {
            if Provider::from_str(&slug).is_none() {
                continue;
            }

            let mut entry = providers_map
                .get(&slug)
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();

            // Write api_key into the JSON slot (or clear it); None = untouched.
            if let Some(k) = p.api_key.as_deref() {
                entry.insert(
                    "api_key".to_string(),
                    serde_json::Value::String(k.trim().to_string()),
                );
            }

            if let Some(m) = p.model.as_deref() {
                let m = m.trim();
                if !m.is_empty() {
                    entry.insert("model".to_string(), serde_json::Value::String(m.to_string()));
                }
            }
            if let Some(b) = p.base_url.as_deref() {
                let b = b.trim();
                if b.is_empty() {
                    entry.insert("base_url".to_string(), serde_json::Value::Null);
                } else {
                    entry.insert("base_url".to_string(), serde_json::Value::String(b.to_string()));
                }
            }
            if let Some(mt) = p.max_tokens {
                entry.insert("max_tokens".to_string(), serde_json::Value::from(mt.clamp(1, 4096)));
            }

            providers_map.insert(slug, serde_json::Value::Object(entry));
        }

        store.set(KEY_PROVIDERS, serde_json::Value::Object(providers_map));
    }

    store
        .save()
        .map_err(|e| AnteError::ApiError(format!("store save error: {e}")))?;

    Ok(())
}

/// Validate an API key for a given provider (defaults to active).
/// Dispatches to the provider-specific test() impl.
#[tauri::command]
pub async fn test_ai_config(
    app: AppHandle,
    payload: TestAiConfigPayload,
) -> Result<AiTestResult, AnteError> {
    run_migration_if_needed(&app);
    let stored = get_config(&app);

    let provider = payload
        .provider
        .as_deref()
        .and_then(Provider::from_str)
        .unwrap_or(stored.active.provider);

    // Resolve key: payload override → JSON store → env var.
    let payload_key = payload
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let store_key = app
        .store(STORE_FILE)
        .ok()
        .and_then(|store| read_provider_key(&store, provider));

    let api_key = payload_key
        .or(store_key)
        .or_else(|| {
            std::env::var(env_var_for(provider))
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .or_else(|| {
            if provider == Provider::Openai {
                std::env::var("OPENAI_API_KEY")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            } else {
                None
            }
        });

    let api_key = match api_key {
        Some(k) => k,
        None => {
            return Ok(AiTestResult {
                ok: false,
                model_count: None,
                error: Some("no api key".to_string()),
            });
        }
    };

    let base_url = payload
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            app.store(STORE_FILE).ok().map(|store| {
                let (_m, b, _mt) = read_provider_slot(&store, provider);
                b
            })
        })
        .unwrap_or_else(|| default_base_url_for(provider).to_string());

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AnteError::ApiError(sanitize_error(e)))?;

    let client = client_for(provider);
    Ok(client.test(&http, &api_key, &base_url).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_error_strips_details() {
        let msg = sanitize_error(
            "reqwest::Error: https://api.openai.com/v1 Authorization: Bearer sk-abc timeout",
        );
        assert_eq!(msg, "request timed out");
    }

    #[test]
    fn sanitize_error_generic_fallback() {
        let msg = sanitize_error("some unusual failure");
        assert_eq!(msg, "request failed");
    }

    #[test]
    fn provider_serde_roundtrip() {
        for p in Provider::all() {
            let ser = serde_json::to_string(&p).unwrap();
            let back: Provider = serde_json::from_str(&ser).unwrap();
            assert_eq!(p, back);
        }
        assert_eq!(
            serde_json::to_string(&Provider::OpenaiCompatible).unwrap(),
            "\"openai-compatible\""
        );
        assert_eq!(Provider::from_str("anthropic"), Some(Provider::Anthropic));
        assert_eq!(Provider::from_str("nope"), None);
    }

    #[test]
    fn openai_parse_sse_content_token() {
        let data = r#"{"choices":[{"delta":{"content":"hello"}}]}"#;
        let out = openai::parse_openai_sse(data);
        assert_eq!(out, ParseOutcome::Token("hello".to_string()));
    }

    #[test]
    fn openai_parse_sse_empty_delta_is_ignored() {
        let data = r#"{"choices":[{"delta":{}}]}"#;
        let out = openai::parse_openai_sse(data);
        assert_eq!(out, ParseOutcome::Ignore);
    }

    #[test]
    fn openai_parse_sse_done_sentinel() {
        let out = openai::parse_openai_sse("[DONE]");
        assert_eq!(out, ParseOutcome::Done);
    }

    #[test]
    fn anthropic_parse_content_block_delta_text() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#;
        let out = anthropic::parse_anthropic_sse("content_block_delta", data);
        assert_eq!(out, ParseOutcome::Token("Hi".to_string()));
    }

    #[test]
    fn anthropic_parse_input_json_delta_is_ignored() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{"}}"#;
        let out = anthropic::parse_anthropic_sse("content_block_delta", data);
        assert_eq!(out, ParseOutcome::Ignore);
    }

    #[test]
    fn anthropic_parse_ping_and_message_start_ignored() {
        let out = anthropic::parse_anthropic_sse("ping", "{}");
        assert_eq!(out, ParseOutcome::Ignore);
        let out = anthropic::parse_anthropic_sse("message_start", r#"{"type":"message_start"}"#);
        assert_eq!(out, ParseOutcome::Ignore);
    }

    #[test]
    fn anthropic_parse_message_stop_done() {
        let out = anthropic::parse_anthropic_sse("message_stop", r#"{"type":"message_stop"}"#);
        assert_eq!(out, ParseOutcome::Done);
    }

    #[test]
    fn anthropic_parse_unknown_event_ignored() {
        let out = anthropic::parse_anthropic_sse("some_future_event", "{}");
        assert_eq!(out, ParseOutcome::Ignore);
    }
}
