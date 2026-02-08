use chaos_core::danmaku::client::DanmakuClient;
use chaos_core::danmaku::model::ConnectOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .ok_or("usage: danmaku_dump <url_or_room_id>")?;

    let client = DanmakuClient::new()?;
    let (session, mut rx) = client.connect(&input, ConnectOptions::default()).await?;

    let cancel = session.cancel_token();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        cancel.cancel();
    });

    while let Some(ev) = rx.recv().await {
        println!("{ev:?}");
    }

    session.stop().await;
    Ok(())
}
