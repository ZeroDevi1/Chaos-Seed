//! OpenAI-compatible LLM client (chat completions).
//!
//! This module intentionally uses `reqwest` directly so we can talk to any OpenAI-compatible
//! provider (including self-hosted / third-party gateways) without binding to a specific SDK.

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningMode {
    Normal,
    Reasoning,
}

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub reasoning_model: Option<String>,
    pub timeout_ms: u64,
    pub default_temperature: f32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            // Your stated default (SiliconFlow OpenAI-compatible gateway).
            model: "THUDM/GLM-4-9B-0414".to_string(),
            reasoning_model: None,
            timeout_ms: 30_000,
            default_temperature: 0.7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub reasoning_mode: ReasoningMode,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub text: String,
    pub raw: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("parse error: {0}")]
    Parse(String),
}

#[derive(Clone)]
pub struct LlmClient {
    cfg: LlmConfig,
    http: reqwest::Client,
}

impl LlmClient {
    pub fn new(cfg: LlmConfig) -> Result<Self, LlmError> {
        let base = cfg.base_url.trim();
        if base.is_empty() {
            return Err(LlmError::InvalidConfig("base_url is empty".into()));
        }
        if cfg.api_key.trim().is_empty() {
            return Err(LlmError::InvalidConfig("api_key is empty".into()));
        }
        if cfg.model.trim().is_empty() {
            return Err(LlmError::InvalidConfig("model is empty".into()));
        }

        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(cfg.timeout_ms.max(1)))
            .build()
            .map_err(|e| LlmError::Http(e.to_string()))?;

        Ok(Self { cfg, http })
    }

    pub fn config(&self) -> &LlmConfig {
        &self.cfg
    }

    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LlmError> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let model = match req.reasoning_mode {
            ReasoningMode::Normal => self.cfg.model.clone(),
            ReasoningMode::Reasoning => self
                .cfg
                .reasoning_model
                .clone()
                .unwrap_or_else(|| self.cfg.model.clone()),
        };

        let mut messages: Vec<serde_json::Value> = Vec::new();
        if let Some(sys) = req.system.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            messages.push(serde_json::json!({
                "role": "system",
                "content": sys,
            }));
        }
        for m in &req.messages {
            let role = m.role.trim();
            let content = m.content.trim();
            if role.is_empty() || content.is_empty() {
                continue;
            }
            messages.push(serde_json::json!({
                "role": role,
                "content": content,
            }));
        }
        if messages.is_empty() {
            return Err(LlmError::InvalidConfig("messages is empty".into()));
        }

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
        });

        let temp = req.temperature.unwrap_or(self.cfg.default_temperature);
        if temp.is_finite() && temp >= 0.0 {
            body["temperature"] = serde_json::json!(temp);
        }
        if let Some(mt) = req.max_tokens {
            if mt > 0 {
                // OpenAI-compatible providers accept either `max_tokens` or `max_completion_tokens`.
                body["max_tokens"] = serde_json::json!(mt);
            }
        }

        // Best-effort reasoning hint (ignored by most providers).
        if matches!(req.reasoning_mode, ReasoningMode::Reasoning) {
            body["reasoning"] = serde_json::json!({ "effort": "medium" });
        }

        let resp = self
            .http
            .post(url)
            .bearer_auth(self.cfg.api_key.trim())
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        let status = resp.status();
        let raw: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| LlmError::Parse(e.to_string()))?;

        if !status.is_success() {
            // Try to extract OpenAI-style error message.
            let msg = raw
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("request failed");
            return Err(LlmError::Provider(format!("{status}: {msg}")));
        }

        let text = raw
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        if text.trim().is_empty() {
            return Err(LlmError::Parse("empty response content".into()));
        }

        Ok(ChatResponse { text, raw })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_validates_config() {
        let err = LlmClient::new(LlmConfig { api_key: "".into(), ..Default::default() })
            .err()
            .expect("expected invalid config error");
        assert!(matches!(err, LlmError::InvalidConfig(_)));
    }
}
