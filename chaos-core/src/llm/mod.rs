//! OpenAI-compatible LLM client (chat completions).
//!
//! This module intentionally uses `reqwest` directly so we can talk to any OpenAI-compatible
//! provider (including self-hosted / third-party gateways) without binding to a specific SDK.

pub mod config_toml;

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

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

    /// Normal 模式下是否启用“思考”（非标准 OpenAI 字段：注入到 extra_body.chat_template_kwargs.enable_thinking）。
    pub enable_thinking_normal: bool,
    /// Reasoning 模式下是否启用“思考”。
    pub enable_thinking_reasoning: bool,
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
            enable_thinking_normal: false,
            enable_thinking_reasoning: true,
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
    #[error("provider error: {status}: {message}")]
    ProviderStatus { status: u16, message: String },
    #[error("parse error: {0}")]
    Parse(String),
}

#[derive(Clone)]
pub struct LlmClient {
    cfg: LlmConfig,
    http: reqwest::Client,
    resolved_base_url: Arc<Mutex<Option<String>>>,
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

        Ok(Self {
            cfg,
            http,
            resolved_base_url: Arc::new(Mutex::new(None)),
        })
    }

    pub fn config(&self) -> &LlmConfig {
        &self.cfg
    }

    pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LlmError> {
        // base_url 支持不带 /v1：优先尝试 `<base>/chat/completions`，若返回 404/405 再尝试 `<base>/v1/chat/completions`。
        //
        // 说明：这里不使用 OpenAI SDK，是为了兼容各类“OpenAI-compatible gateway / self-hosted”实现。
        let model = match req.reasoning_mode {
            ReasoningMode::Normal => self.cfg.model.clone(),
            ReasoningMode::Reasoning => self
                .cfg
                .reasoning_model
                .clone()
                .unwrap_or_else(|| self.cfg.model.clone()),
        };

        let mut messages: Vec<serde_json::Value> = Vec::new();
        if let Some(sys) = req
            .system
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
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

        // 额外参数（非标准字段）：用于 CosyVoice / Qwen 模板下的“思考/非思考”切换。
        //
        // 约定：
        // - Normal => enable_thinking=false（非思考）
        // - Reasoning => enable_thinking=true（思考）
        let enable_thinking = match req.reasoning_mode {
            ReasoningMode::Normal => self.cfg.enable_thinking_normal,
            ReasoningMode::Reasoning => self.cfg.enable_thinking_reasoning,
        };
        // 确保 extra_body 是 object。
        if !body.get("extra_body").is_some_and(|v| v.is_object()) {
            body["extra_body"] = serde_json::json!({});
        }
        body["extra_body"]["chat_template_kwargs"]["enable_thinking"] =
            serde_json::json!(enable_thinking);

        let base0 = {
            let locked = self.resolved_base_url.lock().await;
            locked.clone().unwrap_or_else(|| self.cfg.base_url.trim().to_string())
        };
        let base0 = base0.trim_end_matches('/').to_string();

        let mut tried_v1 = false;
        let mut last_err: Option<LlmError> = None;
        for attempt in 0..2 {
            let base = if attempt == 0 {
                base0.clone()
            } else {
                tried_v1 = true;
                format!("{}/v1", base0.trim_end_matches("/v1"))
            };
            let url = format!("{base}/chat/completions");

            let r = self.send_chat_once(&url, &body).await;
            match r {
                Ok(ok) => {
                    // 缓存“已验证 base_url”，避免每次探测。
                    let mut locked = self.resolved_base_url.lock().await;
                    *locked = Some(base);
                    return Ok(ok);
                }
                Err(e) => {
                    let should_retry_v1 = matches!(
                        &e,
                        LlmError::ProviderStatus { status: 404, .. }
                            | LlmError::ProviderStatus { status: 405, .. }
                    ) && !base0.ends_with("/v1");
                    last_err = Some(e);
                    if attempt == 0 && should_retry_v1 {
                        continue;
                    }
                    break;
                }
            }
        }

        let _ = tried_v1;
        let raw_err = last_err.unwrap_or_else(|| LlmError::Http("request failed".into()));
        Err(raw_err)
    }

    async fn send_chat_once(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<ChatResponse, LlmError> {
        let resp = self
            .http
            .post(url)
            .bearer_auth(self.cfg.api_key.trim())
            .json(body)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;

        let status = resp.status();
        let text_body = resp.text().await.map_err(|e| LlmError::Http(e.to_string()))?;

        let raw: serde_json::Value = match serde_json::from_str(&text_body) {
            Ok(v) => v,
            Err(_) => serde_json::json!({ "_raw": text_body }),
        };

        if !status.is_success() {
            let msg = raw
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    // 非 JSON 或非 OpenAI error shape：给一个截断的文本（避免爆日志）。
                    let s = raw
                        .get("_raw")
                        .and_then(|v| v.as_str())
                        .unwrap_or("request failed");
                    s.chars().take(200).collect::<String>()
                });
            return Err(LlmError::ProviderStatus {
                status: status.as_u16(),
                message: msg,
            });
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
        let err = LlmClient::new(LlmConfig {
            api_key: "".into(),
            ..Default::default()
        })
        .err()
        .expect("expected invalid config error");
        assert!(matches!(err, LlmError::InvalidConfig(_)));
    }
}
