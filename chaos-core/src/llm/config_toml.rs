//! LLM 配置文件（TOML，KV 形式）加载。
//!
//! 约定：
//! - 示例配置：`config/llm.example.toml`（可提交，不含真实密钥）
//! - 本地真实：`config/llm.toml`（应被 .gitignore 忽略）
//!
//! 说明：
//! - 本模块只负责“找到并解析 llm.toml”，不负责网络探测等运行时逻辑。
//! - 读取顺序：`CHAOS_LLM_CONFIG` -> `<exe>/config/llm.toml` -> `<cwd>/config/llm.toml` -> `%APPDATA%/ChaosSeed/llm.toml`

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::LlmConfig;

#[derive(Debug, Clone, Deserialize)]
pub struct LlmTomlConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub reasoning_model: Option<String>,
    pub timeout_ms: Option<u64>,
    pub default_temperature: Option<f64>,

    pub enable_thinking_normal: Option<bool>,
    pub enable_thinking_reasoning: Option<bool>,
}

impl LlmTomlConfig {
    pub fn into_llm_config(self) -> Result<LlmConfig, String> {
        let base_url = self.base_url.unwrap_or_default().trim().to_string();
        let api_key = self.api_key.unwrap_or_default().trim().to_string();
        let model = self.model.unwrap_or_default().trim().to_string();

        if base_url.is_empty() {
            return Err("llm.toml: base_url is empty".into());
        }
        if api_key.is_empty() {
            return Err("llm.toml: api_key is empty".into());
        }
        if model.is_empty() {
            return Err("llm.toml: model is empty".into());
        }

        let reasoning_model = self
            .reasoning_model
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let timeout_ms = self.timeout_ms.unwrap_or(30_000).max(1);
        let default_temperature = self.default_temperature.unwrap_or(0.7).clamp(0.0, 5.0) as f32;

        Ok(LlmConfig {
            base_url,
            api_key,
            model,
            reasoning_model,
            timeout_ms,
            default_temperature,
            enable_thinking_normal: self.enable_thinking_normal.unwrap_or(false),
            enable_thinking_reasoning: self.enable_thinking_reasoning.unwrap_or(true),
        })
    }
}

pub fn default_search_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();

    // 1) env override
    if let Ok(raw) = std::env::var("CHAOS_LLM_CONFIG") {
        let raw = raw.trim();
        if !raw.is_empty() {
            let p = PathBuf::from(raw);
            let abs = if p.is_absolute() {
                p
            } else {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(p)
            };
            out.push(abs);
        }
    }

    // 2) <exe>/config/llm.toml
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("config").join("llm.toml"));
        }
    }

    // 3) <cwd>/config/llm.toml
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("config").join("llm.toml"));
    }

    // 4) %APPDATA%/ChaosSeed/llm.toml (dirs::config_dir)
    if let Some(root) = dirs::config_dir() {
        out.push(root.join("ChaosSeed").join("llm.toml"));
    }

    // 去重（保留顺序）
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

pub fn load_from_path(path: &Path) -> Result<LlmConfig, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("read llm config failed: {}: {e}", path.display()))?;
    let s = String::from_utf8(bytes)
        .map_err(|e| format!("llm.toml must be UTF-8 (no BOM): {}: {e}", path.display()))?;
    let parsed: LlmTomlConfig = toml::from_str(&s)
        .map_err(|e| format!("parse llm.toml failed: {}: {e}", path.display()))?;
    parsed.into_llm_config()
}

/// 自动搜索并加载 llm.toml。
///
/// - Ok(None)：未找到配置文件
/// - Ok(Some((path, cfg)))：成功加载
/// - Err(msg)：找到文件但解析失败等错误
pub fn autoload_llm_config_with_path() -> Result<Option<(PathBuf, LlmConfig)>, String> {
    for p in default_search_paths() {
        if p.exists() {
            let cfg = load_from_path(&p)?;
            return Ok(Some((p, cfg)));
        }
    }
    Ok(None)
}

pub fn autoload_llm_config() -> Result<Option<LlmConfig>, String> {
    Ok(autoload_llm_config_with_path()?.map(|(_, cfg)| cfg))
}
