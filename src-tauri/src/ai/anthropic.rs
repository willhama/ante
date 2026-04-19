//! Anthropic provider (native `/v1/messages` API).
//!
//! Wire format: POST {base_url}/v1/messages with `x-api-key` +
//! `anthropic-version` headers. SSE uses named events:
//! - `content_block_delta`: `{"type":"content_block_delta","index":N,"delta":{"type":"text_delta","text":"..."}}` -> Token
//! - `message_stop`: -> Done
//! - everything else (`message_start`, `content_block_start`, `content_block_stop`,
//!   `ping`, `message_delta`): -> Ignore
//!
//! We only handle `text_delta` inside `content_block_delta`; `input_json_delta`
//! variants (tool use) are ignored - we don't use tools.

use serde::Deserialize;

use super::provider::{extract_provider_error, AiTestResult, ParseOutcome, ProviderClient, StreamParams};
use crate::ai::sanitize_error;

pub struct AnthropicClient;

pub const ANTHROPIC_DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const ANTHROPIC_DEFAULT_MODEL: &str = "claude-haiku-4-5-20251001";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Deserialize)]
struct AnthropicDelta {
    #[serde(default, rename = "type")]
    delta_type: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct ContentBlockDelta {
    #[serde(default)]
    delta: Option<AnthropicDelta>,
}

/// Parse one SSE event from an Anthropic stream. `event_name` is the SSE
/// `event:` field; `data` is the `data:` JSON payload (already trimmed,
/// guaranteed non-empty by the caller).
pub(crate) fn parse_anthropic_sse(event_name: &str, data: &str) -> ParseOutcome {
    match event_name {
        "content_block_delta" => {
            match serde_json::from_str::<ContentBlockDelta>(data) {
                Ok(parsed) => {
                    if let Some(d) = parsed.delta {
                        if d.delta_type == "text_delta" && !d.text.is_empty() {
                            return ParseOutcome::Token(d.text);
                        }
                    }
                    ParseOutcome::Ignore
                }
                Err(_) => ParseOutcome::Ignore,
            }
        }
        "message_stop" => ParseOutcome::Done,
        // message_start, content_block_start, content_block_stop, ping,
        // message_delta, error (handled via HTTP status), etc.
        _ => ParseOutcome::Ignore,
    }
}

#[async_trait::async_trait]
impl ProviderClient for AnthropicClient {
    fn build_request(
        &self,
        client: &reqwest::Client,
        params: &StreamParams<'_>,
    ) -> reqwest::RequestBuilder {
        let url = format!("{}/v1/messages", params.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": params.model,
            "max_tokens": params.max_tokens,
            "system": params.system,
            "messages": [
                {"role": "user", "content": params.user_content},
            ],
            "stream": true,
            "temperature": params.temperature,
        });
        client
            .post(url)
            .header("x-api-key", params.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
    }

    fn parse_sse_data(&self, event_name: &str, data: &str) -> ParseOutcome {
        parse_anthropic_sse(event_name, data)
    }

    /// One-token dry-run message. Costs fractions of a cent per click -
    /// acceptable trade-off for real validation (listing models on
    /// Anthropic's API requires an admin key, not a standard API key).
    async fn test(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        base_url: &str,
    ) -> AiTestResult {
        let base = if base_url.trim().is_empty() {
            ANTHROPIC_DEFAULT_BASE_URL
        } else {
            base_url
        };
        let url = format!("{}/v1/messages", base.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": ANTHROPIC_DEFAULT_MODEL,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "hi"}],
        });
        let response = match client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return AiTestResult {
                    ok: false,
                    model_count: None,
                    error: Some(sanitize_error(e)),
                };
            }
        };

        let status = response.status();
        if !status.is_success() {
            let code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            let msg = extract_provider_error(&body).unwrap_or_else(|| {
                match code {
                    401 => "invalid key",
                    403 => "key rejected",
                    404 => "model not found or no access",
                    429 => "rate limited",
                    _ => "request failed",
                }
                .to_string()
            });
            return AiTestResult {
                ok: false,
                model_count: None,
                error: Some(msg),
            };
        }

        // Anthropic doesn't expose a cheap model-count endpoint to standard
        // API keys. Return ok=true with no count on success.
        AiTestResult {
            ok: true,
            model_count: None,
            error: None,
        }
    }

    fn default_base_url(&self) -> &'static str {
        ANTHROPIC_DEFAULT_BASE_URL
    }

    fn default_model(&self) -> &'static str {
        ANTHROPIC_DEFAULT_MODEL
    }
}
