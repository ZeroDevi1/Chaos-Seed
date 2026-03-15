use std::sync::Mutex as StdMutex;

use chaos_core::llm::config_toml;
use chaos_core::llm::{ChatMessage, ChatRequest, LlmClient, LlmConfig, ReasoningMode};
use httpmock::Method::POST;
use httpmock::MockServer;

static ENV_LOCK: StdMutex<()> = StdMutex::new(());

#[tokio::test]
async fn chat_injects_enable_thinking_into_extra_body() {
    let server = MockServer::start();
    let base = server.base_url();

    // Normal => enable_thinking=false
    let m1 = server.mock(|when, then| {
        when.method(POST)
            .path("/chat/completions")
            // best-effort: match by substring to avoid depending on full JSON equality.
            .body_contains("\"enable_thinking\":false");
        then.status(200).json_body(serde_json::json!({
            "choices": [{ "message": { "content": "ok" } }]
        }));
    });

    let cfg = LlmConfig {
        base_url: base.clone(),
        api_key: "sk-test".to_string(),
        model: "m".to_string(),
        reasoning_model: None,
        timeout_ms: 5_000,
        default_temperature: 0.7,
        enable_thinking_normal: false,
        enable_thinking_reasoning: true,
    };
    let client = LlmClient::new(cfg).expect("client");

    let req = ChatRequest {
        system: None,
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }],
        reasoning_mode: ReasoningMode::Normal,
        temperature: None,
        max_tokens: None,
    };
    let res = client.chat(req).await.expect("chat");
    assert_eq!(res.text, "ok");
    assert_eq!(m1.hits(), 1);
}

#[tokio::test]
async fn base_url_auto_fallback_to_v1_and_cache() {
    let server = MockServer::start();
    let base = server.base_url();

    let no_v1 = server.mock(|when, then| {
        when.method(POST).path("/chat/completions");
        then.status(404).body("not found");
    });

    let v1 = server.mock(|when, then| {
        when.method(POST).path("/v1/chat/completions");
        then.status(200).json_body(serde_json::json!({
            "choices": [{ "message": { "content": "ok-v1" } }]
        }));
    });

    let cfg = LlmConfig {
        base_url: base.clone(), // no /v1 on purpose
        api_key: "sk-test".to_string(),
        model: "m".to_string(),
        reasoning_model: None,
        timeout_ms: 5_000,
        default_temperature: 0.7,
        enable_thinking_normal: false,
        enable_thinking_reasoning: true,
    };
    let client = LlmClient::new(cfg).expect("client");

    let mk_req = || ChatRequest {
        system: None,
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        }],
        reasoning_mode: ReasoningMode::Normal,
        temperature: None,
        max_tokens: None,
    };

    // First call: hit /chat/completions (404) then fallback to /v1/chat/completions (200).
    let r1 = client.chat(mk_req()).await.expect("chat1");
    assert_eq!(r1.text, "ok-v1");
    assert_eq!(no_v1.hits(), 1);
    assert_eq!(v1.hits(), 1);

    // Second call: should reuse cached base (/v1) and avoid hitting the 404 endpoint.
    let r2 = client.chat(mk_req()).await.expect("chat2");
    assert_eq!(r2.text, "ok-v1");
    assert_eq!(no_v1.hits(), 1);
    assert_eq!(v1.hits(), 2);
}

#[test]
fn autoload_llm_toml_via_env_path() {
    let _g = ENV_LOCK.lock().expect("env lock");

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("llm.toml");
    std::fs::write(
        &path,
        r#"
base_url = "http://127.0.0.1:8008"
api_key = "sk-test"
model = "m"
enable_thinking_normal = false
enable_thinking_reasoning = true
"#,
    )
    .expect("write");

    let prev = std::env::var("CHAOS_LLM_CONFIG").ok();
    unsafe {
        std::env::set_var("CHAOS_LLM_CONFIG", path.to_string_lossy().to_string());
    }

    let loaded = config_toml::autoload_llm_config().expect("autoload");
    assert!(loaded.is_some());
    let cfg = loaded.unwrap();
    assert_eq!(cfg.base_url, "http://127.0.0.1:8008");
    assert_eq!(cfg.api_key, "sk-test");
    assert_eq!(cfg.model, "m");
    assert_eq!(cfg.enable_thinking_normal, false);
    assert_eq!(cfg.enable_thinking_reasoning, true);

    match prev {
        Some(v) => unsafe {
            std::env::set_var("CHAOS_LLM_CONFIG", v);
        },
        None => unsafe {
            std::env::remove_var("CHAOS_LLM_CONFIG");
        },
    }
}
