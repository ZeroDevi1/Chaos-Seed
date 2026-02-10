use chaos_daemon::{read_lsp_frame, run_jsonrpc_over_lsp, write_lsp_frame, ChaosService};
use chaos_proto::{
    DanmakuFetchImageParams, DanmakuFetchImageResult, DanmakuMessage, LiveCloseParams, LiveOpenParams,
    LiveOpenResult,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tokio::io::BufReader;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

struct FakeSvc {
    tx: Arc<Mutex<Option<mpsc::UnboundedSender<DanmakuMessage>>>>,
}

impl FakeSvc {
    fn new() -> Self {
        Self {
            tx: Arc::new(Mutex::new(None)),
        }
    }

    fn push_msg(&self, msg: DanmakuMessage) {
        if let Some(tx) = self.tx.lock().expect("tx mutex").as_ref() {
            let _ = tx.send(msg);
        }
    }
}

impl ChaosService for FakeSvc {
    fn version(&self) -> String {
        "0.0.0-test".to_string()
    }

    async fn live_open(
        &self,
        _params: LiveOpenParams,
    ) -> Result<(LiveOpenResult, mpsc::UnboundedReceiver<DanmakuMessage>), String> {
        let (tx, rx) = mpsc::unbounded_channel::<DanmakuMessage>();
        *self.tx.lock().expect("tx mutex") = Some(tx);
        Ok((
            LiveOpenResult {
                session_id: "sess".to_string(),
                site: "bili_live".to_string(),
                room_id: "1".to_string(),
                title: "t".to_string(),
                variant_id: "v".to_string(),
                variant_label: "lbl".to_string(),
                url: "https://example.com/x.m3u8".to_string(),
                backup_urls: vec![],
                referer: Some("https://live.bilibili.com/1/".to_string()),
                user_agent: None,
            },
            rx,
        ))
    }

    async fn live_close(&self, _params: LiveCloseParams) -> Result<(), String> {
        Ok(())
    }

    async fn danmaku_fetch_image(
        &self,
        _params: DanmakuFetchImageParams,
    ) -> Result<DanmakuFetchImageResult, String> {
        Ok(DanmakuFetchImageResult {
            mime: "image/png".to_string(),
            base64: "AA==".to_string(),
            width: Some(24),
        })
    }
}

#[tokio::test]
async fn jsonrpc_request_response_and_notification_flow() {
    let svc = Arc::new(FakeSvc::new());
    let auth = "token";

    let (client, server) = tokio::io::duplex(64 * 1024);

    let svc2 = svc.clone();
    let server_task = tokio::spawn(async move {
        run_jsonrpc_over_lsp(&*svc2, server, auth).await.unwrap();
    });

    let (r, mut w) = tokio::io::split(client);
    let mut br = BufReader::new(r);

    // 1) ping
    let ping = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "daemon.ping",
        "params": { "authToken": auth }
    });
    let ping_bytes = serde_json::to_vec(&ping).unwrap();
    write_lsp_frame(&mut w, &ping_bytes).await.unwrap();
    let resp1 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 1024))
        .await
        .unwrap()
        .unwrap();
    let v1: serde_json::Value = serde_json::from_slice(&resp1).unwrap();
    assert_eq!(v1["id"], 1);
    assert_eq!(v1["result"]["version"], "0.0.0-test");

    // 2) live.open
    let open = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "live.open",
        "params": { "input": "bilibili:1", "preferredQuality": "highest" }
    });
    let open_bytes = serde_json::to_vec(&open).unwrap();
    write_lsp_frame(&mut w, &open_bytes).await.unwrap();
    let resp2 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v2: serde_json::Value = serde_json::from_slice(&resp2).unwrap();
    assert_eq!(v2["id"], 2);
    assert_eq!(v2["result"]["sessionId"], "sess");

    // 3) notification
    svc.push_msg(DanmakuMessage {
        session_id: "sess".to_string(),
        received_at_ms: 1,
        user: "u".to_string(),
        text: "hi".to_string(),
        image_url: None,
        image_width: None,
    });
    let notif = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let vn: serde_json::Value = serde_json::from_slice(&notif).unwrap();
    assert_eq!(vn["method"], "danmaku.message");
    assert_eq!(vn["params"]["sessionId"], "sess");
    assert_eq!(vn["params"]["text"], "hi");

    // 4) fetch image
    let img = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "danmaku.fetchImage",
        "params": { "sessionId": "sess", "url": "https://example.com/a.png" }
    });
    let img_bytes = serde_json::to_vec(&img).unwrap();
    write_lsp_frame(&mut w, &img_bytes).await.unwrap();
    let resp3 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v3: serde_json::Value = serde_json::from_slice(&resp3).unwrap();
    assert_eq!(v3["id"], 3);
    assert_eq!(v3["result"]["base64"], "AA==");

    // 5) close
    let close = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "live.close",
        "params": { "sessionId": "sess" }
    });
    let close_bytes = serde_json::to_vec(&close).unwrap();
    write_lsp_frame(&mut w, &close_bytes).await.unwrap();
    let resp4 = timeout(Duration::from_secs(3), read_lsp_frame(&mut br, 4096))
        .await
        .unwrap()
        .unwrap();
    let v4: serde_json::Value = serde_json::from_slice(&resp4).unwrap();
    assert_eq!(v4["id"], 4);
    assert_eq!(v4["result"]["ok"], true);

    drop(w);
    drop(br);
    let _ = timeout(Duration::from_secs(3), server_task).await;
}
