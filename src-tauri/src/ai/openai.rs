//! OpenAI provider (also reused for arbitrary OpenAI-compatible endpoints).
//!
//! Wire format: POST {base_url}/chat/completions with a standard chat body,
//! SSE events whose `data:` payload is a JSON chunk with `choices[0].delta.content`,
//! terminated by a literal `[DONE]` sentinel.

use serde::Deserialize;

use super::provider::{extract_provider_error, AiTestResult, ParseOutcome, ProviderClient, StreamParams};
use crate::ai::sanitize_error;

pub struct OpenAiClient;
pub struct OpenAiCompatibleClient;

pub const OPENAI_DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const OPENAI_DEFAULT_MODEL: &str = "gpt-4o-mini";

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

/// Shared implementation. `OpenAiCompatibleClient` piggybacks on the same
/// wire format; the only thing that differs is the default base_url/model
/// (the user must supply these for a compatible endpoint).
fn build_openai_request(
    client: &reqwest::Client,
    params: &StreamParams<'_>,
) -> reqwest::RequestBuilder {
    let url = format!("{}/chat/completions", params.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": params.model,
        "stream": true,
        "temperature": params.temperature,
        "max_tokens": params.max_tokens,
        "messages": [
            {"role": "system", "content": params.system},
            {"role": "user",   "content": params.user_content},
        ],
    });
    client
        .post(url)
        .bearer_auth(params.api_key)
        .header("Content-Type", "application/json")
        .json(&body)
}

/// Parse one SSE `data:` payload from an OpenAI stream.
/// OpenAI doesn't use named events, so `event_name` is ignored.
pub(crate) fn parse_openai_sse(data: &str) -> ParseOutcome {
    if data == "[DONE]" {
        return ParseOutcome::Done;
    }
    match serde_json::from_str::<SseEvent>(data) {
        Ok(parsed) => {
            if let Some(choice) = parsed.choices.into_iter().next() {
                if let Some(text) = choice.delta.content {
                    if !text.is_empty() {
                        return ParseOutcome::Token(text);
                    }
                }
            }
            ParseOutcome::Ignore
        }
        Err(_) => ParseOutcome::Ignore,
    }
}

async fn openai_test(
    client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
) -> AiTestResult {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let response = match client.get(&url).bearer_auth(api_key).send().await {
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
                404 => "endpoint not found",
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

    #[derive(Deserialize)]
    struct ModelsResponse {
        #[serde(default)]
        data: Vec<serde_json::Value>,
    }

    match response.json::<ModelsResponse>().await {
        Ok(parsed) => AiTestResult {
            ok: true,
            model_count: Some(parsed.data.len() as u32),
            error: None,
        },
        Err(_) => AiTestResult {
            ok: true,
            model_count: None,
            error: None,
        },
    }
}

#[async_trait::async_trait]
impl ProviderClient for OpenAiClient {
    fn build_request(
        &self,
        client: &reqwest::Client,
        params: &StreamParams<'_>,
    ) -> reqwest::RequestBuilder {
        build_openai_request(client, params)
    }

    fn parse_sse_data(&self, _event_name: &str, data: &str) -> ParseOutcome {
        parse_openai_sse(data)
    }

    async fn test(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        base_url: &str,
    ) -> AiTestResult {
        openai_test(client, api_key, base_url).await
    }

    fn default_base_url(&self) -> &'static str {
        OPENAI_DEFAULT_BASE_URL
    }

    fn default_model(&self) -> &'static str {
        OPENAI_DEFAULT_MODEL
    }
}

#[async_trait::async_trait]
impl ProviderClient for OpenAiCompatibleClient {
    fn build_request(
        &self,
        client: &reqwest::Client,
        params: &StreamParams<'_>,
    ) -> reqwest::RequestBuilder {
        build_openai_request(client, params)
    }

    fn parse_sse_data(&self, _event_name: &str, data: &str) -> ParseOutcome {
        parse_openai_sse(data)
    }

    async fn test(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        base_url: &str,
    ) -> AiTestResult {
        openai_test(client, api_key, base_url).await
    }

    /// Empty - openai-compatible endpoints have no single default. The UI
    /// requires the user to supply `base_url` explicitly.
    fn default_base_url(&self) -> &'static str {
        ""
    }

    /// Empty - the user picks the model; no universal default fits Groq,
    /// Ollama, OpenRouter, LM Studio, etc.
    fn default_model(&self) -> &'static str {
        ""
    }
}
