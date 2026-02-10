use crate::lsp::{read_lsp_frame, write_lsp_frame};
use crate::rpc::{JsonRpcError, JsonRpcResponse, RpcErrorCode};
use chaos_proto::{
    DaemonPingParams, DaemonPingResult, DanmakuFetchImageParams, LiveCloseParams, LiveOpenParams,
    METHOD_DAEMON_PING, METHOD_DANMAKU_FETCH_IMAGE, METHOD_LIVE_CLOSE, METHOD_LIVE_OPEN,
    NOTIF_DANMAKU_MESSAGE,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::future::Future;
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::sync::mpsc;

pub trait ChaosService: Send + Sync + 'static {
    fn version(&self) -> String;

    fn live_open(
        &self,
        params: LiveOpenParams,
    ) -> impl Future<
        Output = Result<
            (
                chaos_proto::LiveOpenResult,
                mpsc::UnboundedReceiver<chaos_proto::DanmakuMessage>,
            ),
            String,
        >,
    > + Send;

    fn live_close(&self, params: LiveCloseParams) -> impl Future<Output = Result<(), String>> + Send;

    fn danmaku_fetch_image(
        &self,
        params: DanmakuFetchImageParams,
    ) -> impl Future<Output = Result<chaos_proto::DanmakuFetchImageResult, String>> + Send;
}

#[derive(Debug, serde::Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

pub async fn run_jsonrpc_over_lsp<S: ChaosService, RW: AsyncRead + AsyncWrite + Unpin>(
    svc: &S,
    rw: RW,
    auth_token: &str,
) -> Result<(), crate::LspFrameError> {
    let (r, mut w) = tokio::io::split(rw);
    let mut br = BufReader::new(r);

    let mut authed = false;
    let mut active_session_id: Option<String> = None;
    let mut notif_rx: Option<mpsc::UnboundedReceiver<chaos_proto::DanmakuMessage>> = None;

    loop {
        tokio::select! {
            biased;

            Some(msg) = async {
                if let Some(rx) = notif_rx.as_mut() {
                    rx.recv().await
                } else {
                    None
                }
            }, if notif_rx.is_some() => {
                let payload = json!({
                    "jsonrpc": "2.0",
                    "method": NOTIF_DANMAKU_MESSAGE,
                    "params": msg,
                });
                let bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{}".to_vec());
                let _ = write_lsp_frame(&mut w, &bytes).await;
            }

            frame = read_lsp_frame(&mut br, 4 * 1024 * 1024) => {
                let frame = frame?;
                let req: JsonRpcRequest = match serde_json::from_slice(&frame) {
                    Ok(v) => v,
                    Err(_) => {
                        // Parse error: cannot reply without an id; drop.
                        continue;
                    }
                };

                if req.jsonrpc != "2.0" {
                    if let Some(id) = req.id {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InvalidRequest, "invalid jsonrpc version"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                    }
                    continue;
                }

                let Some(id) = req.id else {
                    // Notification from client: ignore for PoC.
                    continue;
                };

                if !authed {
                    if req.method != METHOD_DAEMON_PING {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::Unauthorized, "not authenticated"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                        continue;
                    }
                    let params: DaemonPingParams = match decode_params(req.params) {
                        Ok(v) => v,
                        Err(e) => {
                            let resp = JsonRpcResponse::err(id, e);
                            let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                            let _ = write_lsp_frame(&mut w, &bytes).await;
                            continue;
                        }
                    };
                    if params.auth_token != auth_token {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::Unauthorized, "invalid auth token"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                        break;
                    }
                    authed = true;
                    let result = DaemonPingResult { version: svc.version() };
                    let resp = JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap());
                    let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                    let _ = write_lsp_frame(&mut w, &bytes).await;
                    continue;
                }

                match req.method.as_str() {
                    METHOD_DAEMON_PING => {
                        let result = DaemonPingResult { version: svc.version() };
                        let resp = JsonRpcResponse::ok(id, serde_json::to_value(result).unwrap());
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                    }
                    METHOD_LIVE_OPEN => {
                        if let Some(prev) = active_session_id.take() {
                            let _ = svc.live_close(LiveCloseParams { session_id: prev }).await;
                            notif_rx = None;
                        }
                        let params: LiveOpenParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.live_open(params).await {
                            Ok((res, rx)) => {
                                active_session_id = Some(res.session_id.clone());
                                notif_rx = Some(rx);
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_LIVE_CLOSE => {
                        let params: LiveCloseParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        let sid = params.session_id.clone();
                        match svc.live_close(params).await {
                            Ok(()) => {
                                if active_session_id.as_deref() == Some(&sid) {
                                    active_session_id = None;
                                    notif_rx = None;
                                }
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(chaos_proto::OkReply { ok: true }).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    METHOD_DANMAKU_FETCH_IMAGE => {
                        let params: DanmakuFetchImageParams = match decode_params(req.params) {
                            Ok(v) => v,
                            Err(e) => {
                                let resp = JsonRpcResponse::err(id, e);
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                                continue;
                            }
                        };
                        match svc.danmaku_fetch_image(params).await {
                            Ok(res) => {
                                let resp = JsonRpcResponse::ok(id, serde_json::to_value(res).unwrap());
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                            Err(msg) => {
                                let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::InternalError, msg));
                                let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                                let _ = write_lsp_frame(&mut w, &bytes).await;
                            }
                        }
                    }
                    _ => {
                        let resp = JsonRpcResponse::err(id, JsonRpcError::new(RpcErrorCode::MethodNotFound, "method not found"));
                        let bytes = serde_json::to_vec(&resp).unwrap_or_else(|_| b"{}".to_vec());
                        let _ = write_lsp_frame(&mut w, &bytes).await;
                    }
                }
            }
        }
    }

    Ok(())
}

fn decode_params<T: DeserializeOwned>(params: Option<Value>) -> Result<T, JsonRpcError> {
    let Some(p) = params else {
        return Err(JsonRpcError::new(RpcErrorCode::InvalidParams, "missing params"));
    };
    serde_json::from_value::<T>(p)
        .map_err(|_| JsonRpcError::new(RpcErrorCode::InvalidParams, "invalid params"))
}
