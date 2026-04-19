//! Provider trait and shared types for multi-provider AI streaming.
//!
//! Each concrete provider (OpenAI, Anthropic, ...) implements `ProviderClient`
//! to encode its wire format: how to build the HTTP request and how to parse
//! SSE events into normalized `ParseOutcome` values.
//!
//! The streaming loop in `ai::run_stream` is provider-agnostic - it calls
//! `build_request`/`parse_sse_data` against whichever `Box<dyn ProviderClient>`
//! was selected for the active provider.

use serde::{Deserialize, Serialize};

/// All supported providers. Serialized kebab-case to match the frontend
/// (`'openai' | 'openai-compatible' | 'anthropic'`).
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Openai,
    OpenaiCompatible,
    Anthropic,
}

impl Provider {
    /// Stable kebab-case string for store keys and keychain entries.
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Openai => "openai",
            Provider::OpenaiCompatible => "openai-compatible",
            Provider::Anthropic => "anthropic",
        }
    }

    /// Parse from the kebab-case form. Unknown strings -> None.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "openai" => Some(Provider::Openai),
            "openai-compatible" => Some(Provider::OpenaiCompatible),
            "anthropic" => Some(Provider::Anthropic),
            _ => None,
        }
    }

    pub fn all() -> [Provider; 3] {
        [Provider::Openai, Provider::OpenaiCompatible, Provider::Anthropic]
    }
}

/// Normalized parameters passed to every provider's request builder.
pub struct StreamParams<'a> {
    pub api_key: &'a str,
    pub base_url: &'a str,
    pub model: &'a str,
    pub max_tokens: u32,
    pub system: &'a str,
    pub user_content: &'a str,
    pub temperature: f32,
}

/// Result of parsing one SSE event from a provider.
#[derive(Debug, PartialEq, Eq)]
pub enum ParseOutcome {
    /// A text fragment to append to the visible suggestion.
    Token(String),
    /// The stream is finished; emit `completion-done` and stop reading.
    Done,
    /// Provider-specific event we don't care about (pings, starts, etc.).
    Ignore,
}

/// Result shape for provider `test()` calls, surfaced verbatim to the UI.
#[derive(Serialize, Debug)]
pub struct AiTestResult {
    pub ok: bool,
    pub model_count: Option<u32>,
    pub error: Option<String>,
}

/// Extract a human-readable error message from an OpenAI- or Anthropic-shape
/// error body: `{"error": {"message": "..."}}`. Returns the message trimmed
/// to 240 chars. Safe to surface to the UI: does not include URLs, keys, or
/// the user's prompt (both providers return natural-language strings here).
pub fn extract_provider_error(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct ErrWrap {
        error: ErrInner,
    }
    #[derive(Deserialize)]
    struct ErrInner {
        #[serde(default)]
        message: String,
    }
    serde_json::from_str::<ErrWrap>(body)
        .ok()
        .map(|e| e.error.message)
        .filter(|m| !m.is_empty())
        .map(|m| {
            let trimmed = m.trim();
            if trimmed.chars().count() > 240 {
                trimmed.chars().take(237).collect::<String>() + "..."
            } else {
                trimmed.to_string()
            }
        })
}

/// Abstracts the wire format of a streaming chat completion.
///
/// All methods take &self so the impl stays stateless and cheap to
/// box (`Box<dyn ProviderClient>`).
#[async_trait::async_trait]
pub trait ProviderClient: Send + Sync {
    /// Build the HTTP POST that opens the SSE stream. Caller will
    /// `.send()` it; this method must have set auth headers, body,
    /// and any provider-specific headers (e.g. `anthropic-version`).
    fn build_request(
        &self,
        client: &reqwest::Client,
        params: &StreamParams<'_>,
    ) -> reqwest::RequestBuilder;

    /// Parse one SSE event into a normalized outcome.
    ///
    /// `event_name` is the SSE `event:` field (empty for OpenAI, named
    /// events like `content_block_delta` for Anthropic). `data` is the
    /// SSE `data:` field (trimmed; never empty when called).
    fn parse_sse_data(&self, event_name: &str, data: &str) -> ParseOutcome;

    /// Cheap validity check for a key + base_url. Never leaks the key
    /// back into the returned structure.
    async fn test(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        base_url: &str,
    ) -> AiTestResult;

    /// Default endpoint for this provider when user hasn't overridden.
    #[allow(dead_code)] // Exposed for future UI auto-fill, but ai.rs uses the const defaults directly.
    fn default_base_url(&self) -> &'static str;

    /// Default model slug for this provider.
    #[allow(dead_code)] // Exposed for future UI auto-fill, but ai.rs uses the const defaults directly.
    fn default_model(&self) -> &'static str;
}
