use crate::errors::AnteError;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_MODEL: &str = "gpt-4o-mini";
const SYSTEM_PROMPT: &str = "You complete the user's text. Output only the continuation, no explanations, no quotes, no markdown. Continue naturally for 1-2 sentences, matching the user's style and tone.";
const MAX_TOKENS: u32 = 80;
const TEMPERATURE: f32 = 0.3;

/// Resolved AI configuration from environment variables.
#[derive(Clone)]
struct AiConfig {
    api_key: Option<String>,
    base_url: String,
    model: String,
}

/// Status payload returned to the frontend. Never includes the API key.
#[derive(Serialize)]
pub struct AiStatus {
    pub enabled: bool,
    pub model: String,
}

/// Tracks cancellation senders for in-flight completion streams.
#[derive(Default)]
pub struct AiState {
    cancellers: Mutex<HashMap<String, oneshot::Sender<()>>>,
}

/// Read configuration once from env vars. Cached for the process lifetime.
fn get_config() -> &'static AiConfig {
    static CONFIG: OnceLock<AiConfig> = OnceLock::new();
    CONFIG.get_or_init(|| {
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
        }
    })
}

/// Sanitize error messages so we never leak API key, URLs, or headers to the UI.
fn sanitize_error(raw: impl std::fmt::Display) -> String {
    let s = raw.to_string();
    // Keep it generic. Raw reqwest errors can contain headers and URLs.
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
    let config = get_config().clone();
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
            context_before,
            context_after,
            cancel_rx,
        )
        .await;

        // Clean up the canceller entry once the task finishes.
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
    context_before: String,
    context_after: String,
    mut cancel_rx: oneshot::Receiver<()>,
) {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let user_content = format!("{}{{CURSOR}}{}", context_before, context_after);

    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "temperature": TEMPERATURE,
        "max_tokens": MAX_TOKENS,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
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
        // Don't forward the body; could contain detail useful to an attacker.
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
                                // Skip malformed lines silently. Some providers emit
                                // non-JSON keepalive events.
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

/// Returns whether the AI feature is enabled (API key present) and the configured model.
#[tauri::command]
pub fn get_ai_config() -> AiStatus {
    let config = get_config();
    AiStatus {
        enabled: config.api_key.is_some(),
        model: config.model.clone(),
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
