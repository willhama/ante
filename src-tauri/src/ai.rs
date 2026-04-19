use crate::errors::AnteError;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use tokio::sync::oneshot;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const STORE_FILE: &str = "ai-config.json";
const KEY_API_KEY: &str = "api_key";
const KEY_MODEL: &str = "model";
const KEY_BASE_URL: &str = "base_url";
const KEY_MAX_TOKENS: &str = "max_tokens";
const KEY_TRIGGER_SPEED: &str = "trigger_speed";
const SYSTEM_PROMPT_BASE: &str = "You complete the user's text. Output only the continuation, no explanations, no quotes, no markdown. Match the user's style and tone.";

/// Map max_tokens to a length hint the model actually respects.
/// Chat models follow prompt instructions more reliably than max_tokens alone.
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

/// Resolved AI configuration.
#[derive(Clone)]
struct AiConfig {
    api_key: Option<String>,
    base_url: String,
    model: String,
    max_tokens: u32,
    trigger_speed: String,
}

/// Status payload returned to the frontend. Never includes the API key.
#[derive(Serialize)]
pub struct AiStatus {
    pub enabled: bool,
    pub model: String,
}

/// Payload for load_ai_config - metadata only, never the key itself.
#[derive(Serialize)]
pub struct AiConfigMeta {
    pub has_key: bool,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub trigger_speed: String,
}

/// Payload received when saving config from the frontend.
#[derive(Deserialize)]
pub struct SaveAiConfigPayload {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: Option<u32>,
    pub trigger_speed: Option<String>,
}

/// Payload received when testing a key that hasn't been persisted yet.
#[derive(Deserialize)]
pub struct TestAiConfigPayload {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
}

/// Result of a test call. Never leaks the key back.
#[derive(Serialize)]
pub struct AiTestResult {
    pub ok: bool,
    pub model_count: Option<u32>,
    pub error: Option<String>,
}

/// Tracks cancellation senders for in-flight completion streams.
#[derive(Default)]
pub struct AiState {
    cancellers: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

/// Read config from store, with env-var fallback.
/// Called fresh on every stream so runtime changes take effect immediately.
fn get_config(app: &AppHandle) -> AiConfig {
    // Try store first
    if let Ok(store) = app.store(STORE_FILE) {
        let api_key = store
            .get(KEY_API_KEY)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                std::env::var("OPENAI_API_KEY")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            });

        let base_url = store
            .get(KEY_BASE_URL)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                std::env::var("OPENAI_BASE_URL")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            })
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let model = store
            .get(KEY_MODEL)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                std::env::var("OPENAI_MODEL")
                    .ok()
                    .filter(|v| !v.trim().is_empty())
            })
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        let max_tokens = store
            .get(KEY_MAX_TOKENS)
            .and_then(|v| v.as_u64())
            .map(|n| n.clamp(1, 4096) as u32)
            .unwrap_or(DEFAULT_MAX_TOKENS);

        let trigger_speed = store
            .get(KEY_TRIGGER_SPEED)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| matches!(s.as_str(), "eager" | "balanced" | "relaxed"))
            .unwrap_or_else(|| DEFAULT_TRIGGER_SPEED.to_string());

        return AiConfig {
            api_key,
            base_url,
            model,
            max_tokens,
            trigger_speed,
        };
    }

    // Pure env-var fallback (store plugin unavailable in dev without context)
    let api_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let base_url = std::env::var("OPENAI_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let model = std::env::var("OPENAI_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());

    AiConfig {
        api_key,
        base_url,
        model,
        max_tokens: DEFAULT_MAX_TOKENS,
        trigger_speed: DEFAULT_TRIGGER_SPEED.to_string(),
    }
}

/// Sanitize error messages so we never leak API key, URLs, or headers to the UI.
fn sanitize_error(raw: impl std::fmt::Display) -> String {
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

/// Partial SSE delta shape from OpenAI Chat Completions.
#[derive(Deserialize)]
struct SseChoice {
    #[serde(default)]
    delta: SseDelta,
}

#[derive(Deserialize, Default)]
struct SseDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct SseEvent {
    #[serde(default)]
    choices: Vec<SseChoice>,
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
    let api_key = match config.api_key.clone() {
        Some(k) => k,
        None => return Err(AnteError::ApiError("not configured".to_string())),
    };

    let request_id = uuid::Uuid::new_v4().to_string();
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

    {
        let mut map = state.cancellers.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(request_id.clone(), cancel_tx);
    }

    let app_clone = app.clone();
    let req_id = request_id.clone();

    tokio::spawn(async move {
        run_stream(
            app_clone.clone(),
            req_id.clone(),
            api_key,
            config.base_url,
            config.model,
            config.max_tokens,
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

/// The main streaming loop. Emits chunk/done/error events. Respects cancellation.
#[allow(clippy::too_many_arguments)]
async fn run_stream(
    app: AppHandle,
    id: String,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: u32,
    context_before: String,
    context_after: String,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let user_content = format!("{}{{CURSOR}}{}", context_before, context_after);

    let system_prompt = format!("{} {}", SYSTEM_PROMPT_BASE, length_hint(max_tokens));

    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "temperature": TEMPERATURE,
        "max_tokens": max_tokens,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content}
        ]
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            emit_error(&app, &id, sanitize_error(e));
            return;
        }
    };

    let request_fut = client
        .post(&url)
        .bearer_auth(&api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send();

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
        let status = response.status();
        let msg = if status.as_u16() == 401 {
            "auth failed".to_string()
        } else if status.as_u16() == 429 {
            "rate limited".to_string()
        } else {
            format!("http {}", status.as_u16())
        };
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
                        if data == "[DONE]" {
                            let _ = app.emit(
                                "completion-done",
                                DonePayload { id: id.clone() },
                            );
                            return;
                        }
                        match serde_json::from_str::<SseEvent>(data) {
                            Ok(parsed) => {
                                if let Some(choice) = parsed.choices.into_iter().next() {
                                    if let Some(text) = choice.delta.content {
                                        if !text.is_empty() {
                                            let _ = app.emit(
                                                "completion-chunk",
                                                ChunkPayload {
                                                    id: id.clone(),
                                                    text,
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                continue;
                            }
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

/// Returns whether the AI feature is enabled and the configured model.
/// Reads fresh from store + env each call so UI reflects persisted state.
#[tauri::command]
pub fn get_ai_config(app: AppHandle) -> AiStatus {
    let config = get_config(&app);
    AiStatus {
        enabled: config.api_key.is_some(),
        model: config.model,
    }
}

/// Returns AI config metadata for the settings UI. Never returns the key value.
#[tauri::command]
pub fn load_ai_config(app: AppHandle) -> AiConfigMeta {
    let config = get_config(&app);
    AiConfigMeta {
        has_key: config.api_key.is_some(),
        model: config.model,
        base_url: config.base_url,
        max_tokens: config.max_tokens,
        trigger_speed: config.trigger_speed,
    }
}

/// Persists AI configuration to the store.
/// The api_key field semantics:
///   None           -> leave the stored key untouched (user did not edit it).
///   Some("")       -> explicitly clear the stored key.
///   Some("sk-...") -> replace the stored key with this value.
#[tauri::command]
pub fn save_ai_config(
    app: AppHandle,
    payload: SaveAiConfigPayload,
) -> Result<(), AnteError> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AnteError::ApiError(format!("store error: {e}")))?;

    match payload.api_key.as_deref() {
        Some(k) if !k.trim().is_empty() => {
            store.set(KEY_API_KEY, serde_json::Value::String(k.trim().to_string()));
        }
        Some(_) => {
            // Empty string = explicit clear.
            store.delete(KEY_API_KEY);
        }
        None => {
            // Not provided = leave untouched. Do nothing.
        }
    }

    if let Some(model) = payload.model.as_deref() {
        let m = model.trim();
        if !m.is_empty() {
            store.set(KEY_MODEL, serde_json::Value::String(m.to_string()));
        }
    }

    if let Some(base_url) = payload.base_url.as_deref() {
        let b = base_url.trim();
        if !b.is_empty() {
            store.set(KEY_BASE_URL, serde_json::Value::String(b.to_string()));
        }
    }

    if let Some(mt) = payload.max_tokens {
        let clamped = mt.clamp(1, 4096);
        store.set(KEY_MAX_TOKENS, serde_json::Value::from(clamped));
    }

    if let Some(speed) = payload.trigger_speed.as_deref() {
        if matches!(speed, "eager" | "balanced" | "relaxed") {
            store.set(KEY_TRIGGER_SPEED, serde_json::Value::String(speed.to_string()));
        }
    }

    store
        .save()
        .map_err(|e| AnteError::ApiError(format!("store save error: {e}")))?;

    Ok(())
}

/// Validate an API key by calling GET {base_url}/models. Cheap, no tokens.
/// Uses the provided key/base_url if present; otherwise falls back to stored config.
/// Returns model_count on success. OpenAI does not expose credit balance to
/// standard sk-... keys (billing endpoints require dashboard/admin credentials),
/// so remaining credits are not returned.
#[tauri::command]
pub async fn test_ai_config(
    app: AppHandle,
    payload: TestAiConfigPayload,
) -> Result<AiTestResult, AnteError> {
    let stored = get_config(&app);

    let api_key = payload
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or(stored.api_key);

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
        .unwrap_or(stored.base_url);

    let url = format!("{}/models", base_url.trim_end_matches('/'));

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AnteError::ApiError(sanitize_error(e)))?;

    let response = match client.get(&url).bearer_auth(&api_key).send().await {
        Ok(r) => r,
        Err(e) => {
            return Ok(AiTestResult {
                ok: false,
                model_count: None,
                error: Some(sanitize_error(e)),
            });
        }
    };

    let status = response.status();
    if !status.is_success() {
        let msg = match status.as_u16() {
            401 => "invalid key",
            403 => "key rejected",
            404 => "endpoint not found",
            429 => "rate limited",
            _ => "request failed",
        };
        return Ok(AiTestResult {
            ok: false,
            model_count: None,
            error: Some(msg.to_string()),
        });
    }

    #[derive(Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Vec<serde_json::Value>,
    }

    match response.json::<ModelsResponse>().await {
        Ok(parsed) => Ok(AiTestResult {
            ok: true,
            model_count: Some(parsed.data.len() as u32),
            error: None,
        }),
        Err(_) => Ok(AiTestResult {
            ok: true,
            model_count: None,
            error: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_error_strips_details() {
        let msg = sanitize_error("reqwest::Error: https://api.openai.com/v1 Authorization: Bearer sk-abc timeout");
        assert_eq!(msg, "request timed out");
    }

    #[test]
    fn sanitize_error_generic_fallback() {
        let msg = sanitize_error("some unusual failure");
        assert_eq!(msg, "request failed");
    }
}
