use chaos_core::danmaku::model::{DanmakuComment, DanmakuEvent, DanmakuMethod};
use chaos_core::danmaku::text::sanitize_danmaku_text;
use chaos_proto::DanmakuMessage;
use std::collections::HashSet;

pub fn map_event_to_proto(session_id: String, ev: DanmakuEvent) -> Vec<DanmakuMessage> {
    if ev.method != DanmakuMethod::SendDM {
        return Vec::new();
    }

    let user = if ev.user.trim().is_empty() {
        ev.site.as_str().to_string()
    } else {
        sanitize_danmaku_text(ev.user.trim())
    };

    fn push_comment(
        out: &mut Vec<DanmakuMessage>,
        seen: &mut HashSet<String>,
        session_id: &str,
        ev: &DanmakuEvent,
        user: &str,
        c: &DanmakuComment,
    ) {
        let mut text = c.text.trim().to_string();
        let image_url = c
            .image_url
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        if text.is_empty() && image_url.is_some() {
            text = "[表情]".to_string();
        }
        text = sanitize_danmaku_text(text.trim());

        if text.is_empty() && image_url.is_none() {
            return;
        }

        let key = format!(
            "{}\n{}\n{}",
            text,
            image_url.unwrap_or(""),
            c.image_width.unwrap_or(0)
        );
        if !seen.insert(key) {
            return;
        }

        out.push(DanmakuMessage {
            session_id: session_id.to_string(),
            received_at_ms: ev.received_at_ms,
            user: user.to_string(),
            text,
            image_url: image_url
                .map(|s| s.to_string())
                .or_else(|| c.image_url.clone()),
            image_width: c.image_width,
        });
    }

    let mut out = Vec::<DanmakuMessage>::new();
    let mut seen = HashSet::<String>::new();
    if let Some(dms) = ev.dms.as_ref() {
        for c in dms {
            push_comment(&mut out, &mut seen, &session_id, &ev, &user, c);
        }
    }

    if out.is_empty() {
        let text = sanitize_danmaku_text(ev.text.trim());
        if !text.is_empty() {
            out.push(DanmakuMessage {
                session_id,
                received_at_ms: ev.received_at_ms,
                user,
                text,
                image_url: None,
                image_width: None,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::map_event_to_proto;
    use chaos_core::danmaku::model::{DanmakuComment, DanmakuEvent, DanmakuMethod, Site};

    #[test]
    fn ignores_non_senddm() {
        let ev = DanmakuEvent::new(Site::BiliLive, "1", DanmakuMethod::LiveDMServer, "", None);
        assert!(map_event_to_proto("s".to_string(), ev).is_empty());
    }

    #[test]
    fn maps_placeholder_for_image_only() {
        let mut ev = DanmakuEvent::new(Site::BiliLive, "1", DanmakuMethod::SendDM, "", None);
        ev.dms = Some(vec![DanmakuComment {
            text: "".to_string(),
            image_url: Some("https://example.com/a.png".to_string()),
            image_width: Some(64),
        }]);
        let out = map_event_to_proto("s".to_string(), ev);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "[表情]");
        assert!(out[0].image_url.as_deref().unwrap().contains("example.com"));
    }
}

