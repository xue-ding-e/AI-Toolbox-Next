mod codex_history;
mod gemini_shadow;
mod responses_cipher;

use std::sync::Arc;

pub(super) use codex_history::record_responses_sse_stream;
pub(super) use gemini_shadow::{record_gemini_sse_stream, GeminiShadowSessionKey};

#[derive(Clone, Default)]
pub(super) struct GatewaySideStores {
    codex_history: Arc<codex_history::CodexHistoryStore>,
    gemini_shadow: Arc<gemini_shadow::GeminiShadowStore>,
    invalid_responses_ciphers: Arc<responses_cipher::InvalidResponsesCipherStore>,
}

impl GatewaySideStores {
    pub(super) fn enrich_codex_request(&self, body: &mut serde_json::Value) -> usize {
        self.codex_history.enrich_request(body)
    }

    pub(super) fn record_codex_response(&self, response: &serde_json::Value) -> usize {
        self.codex_history.record_response(response)
    }

    pub(super) fn codex_history(&self) -> Arc<codex_history::CodexHistoryStore> {
        self.codex_history.clone()
    }

    pub(super) fn enrich_gemini_request(
        &self,
        key: &GeminiShadowSessionKey,
        body: &mut serde_json::Value,
    ) -> usize {
        self.gemini_shadow.enrich_request(key, body)
    }

    pub(super) fn record_gemini_response(
        &self,
        key: GeminiShadowSessionKey,
        response: &serde_json::Value,
    ) -> usize {
        self.gemini_shadow.record_response(key, response)
    }

    pub(super) fn gemini_shadow(&self) -> Arc<gemini_shadow::GeminiShadowStore> {
        self.gemini_shadow.clone()
    }

    pub(super) fn remember_invalid_responses_ciphers(
        &self,
        provider_id: &str,
        body: &[u8],
    ) -> usize {
        self.invalid_responses_ciphers
            .remember_from_body(provider_id, body)
    }

    pub(super) fn strip_known_invalid_responses_ciphers(
        &self,
        provider_id: &str,
        body: &mut serde_json::Value,
    ) -> usize {
        self.invalid_responses_ciphers
            .strip_known_from_body(provider_id, body)
    }
}
