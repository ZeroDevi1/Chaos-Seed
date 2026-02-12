use chaos_core::danmaku::model::Site;
use chaos_proto::DanmakuFetchImageResult;
use url::Host;

use crate::ChaosAppError;

use base64::Engine;

pub fn image_referer(
    site: Option<Site>,
    room_id: Option<String>,
    url: &url::Url,
) -> Option<String> {
    let host = url.host_str().unwrap_or_default().to_lowercase();
    let site = site
        .map(|s| s.as_str().to_string())
        .unwrap_or_default()
        .to_lowercase();
    let room_id = room_id.unwrap_or_default();

    if site.contains("bili") || host.contains("bilibili.com") || host.contains("hdslb.com") {
        if room_id.trim().is_empty() {
            return Some("https://live.bilibili.com/".to_string());
        }
        return Some(format!("https://live.bilibili.com/{}/", room_id.trim()));
    }
    None
}

pub fn is_local_or_private_host(u: &url::Url) -> bool {
    let Some(host) = u.host() else {
        return true;
    };

    match host {
        Host::Domain(d) => {
            let h = d.to_lowercase();
            h == "localhost"
        }
        Host::Ipv4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        Host::Ipv6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

pub fn encode_image_reply(
    bytes: &[u8],
    mime: &str,
    width: Option<u32>,
) -> Result<DanmakuFetchImageResult, ChaosAppError> {
    let b64 = base64::engine::general_purpose::STANDARD
        .encode(bytes)
        .trim()
        .to_string();
    Ok(DanmakuFetchImageResult {
        mime: mime.to_string(),
        base64: b64,
        width,
    })
}

#[cfg(test)]
mod tests {
    use super::{image_referer, is_local_or_private_host};
    use chaos_core::danmaku::model::Site;

    fn parse(url: &str) -> url::Url {
        url::Url::parse(url).expect("valid url")
    }

    #[test]
    fn blocks_localhost_and_private_ipv4() {
        assert!(is_local_or_private_host(&parse(
            "http://localhost:8080/a.png"
        )));
        assert!(is_local_or_private_host(&parse("http://127.0.0.1/a.png")));
        assert!(is_local_or_private_host(&parse(
            "http://192.168.1.10/a.png"
        )));
        assert!(is_local_or_private_host(&parse("http://10.0.0.5/a.png")));
    }

    #[test]
    fn allows_public_hosts() {
        assert!(!is_local_or_private_host(&parse(
            "https://example.com/a.png"
        )));
        assert!(!is_local_or_private_host(&parse("http://8.8.8.8/a.png")));
    }

    #[test]
    fn bili_referer_uses_room_id() {
        let u = parse("https://i0.hdslb.com/bfs/emote/a.png");
        let r = image_referer(Some(Site::BiliLive), Some("123".to_string()), &u).unwrap();
        assert!(r.contains("live.bilibili.com/123"));
    }
}
