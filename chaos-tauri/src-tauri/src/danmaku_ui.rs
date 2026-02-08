use chaos_core::danmaku::model::{DanmakuComment, DanmakuEvent, DanmakuMethod};
use std::collections::HashSet;

#[derive(Debug, Clone, serde::Serialize)]
pub struct DanmakuUiMessage {
    pub site: String,
    pub room_id: String,
    pub received_at_ms: i64,
    pub user: String,
    pub text: String,
    pub image_url: Option<String>,
    pub image_width: Option<u32>,
}

pub fn map_event_to_ui(ev: DanmakuEvent) -> Vec<DanmakuUiMessage> {
    if ev.method != DanmakuMethod::SendDM {
        return Vec::new();
    }

    let user = if ev.user.trim().is_empty() {
        ev.site.as_str().to_string()
    } else {
        ev.user.clone()
    };

    fn push_comment(
        out: &mut Vec<DanmakuUiMessage>,
        seen: &mut HashSet<String>,
        ev: &DanmakuEvent,
        user: &str,
        c: &DanmakuComment,
    ) {
        let text = c.text.trim().to_string();
        let image_url = c.image_url.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty());

        // If we only have an image, keep a stable placeholder text so the UI can show a row.
        let text = if text.is_empty() && image_url.is_some() {
            "[表情]".to_string()
        } else {
            text
        };

        if text.is_empty() && image_url.is_none() {
            return;
        }

        // Some platforms/payloads may contain duplicated comment entries.
        // Deduplicate within a single core event to avoid triple-rendering in UI.
        let key = format!(
            "{}\n{}\n{}",
            text,
            image_url.unwrap_or(""),
            c.image_width.unwrap_or(0)
        );
        if !seen.insert(key) {
            return;
        }

        out.push(DanmakuUiMessage {
            site: ev.site.as_str().to_string(),
            room_id: ev.room_id.clone(),
            received_at_ms: ev.received_at_ms,
            user: user.to_string(),
            text,
            image_url: image_url.map(|s| s.to_string()).or_else(|| c.image_url.clone()),
            image_width: c.image_width,
        });
    }

    let mut out = Vec::<DanmakuUiMessage>::new();
    let mut seen = HashSet::<String>::new();
    if let Some(dms) = ev.dms.as_ref() {
        for c in dms {
            push_comment(&mut out, &mut seen, &ev, &user, c);
        }
    }

    // Fallback for platforms/payloads that don't populate `dms`.
    if out.is_empty() {
        let text = ev.text.trim().to_string();
        if !text.is_empty() {
            out.push(DanmakuUiMessage {
                site: ev.site.as_str().to_string(),
                room_id: ev.room_id.clone(),
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
    use super::*;
    use chaos_core::danmaku::model::{DanmakuComment, DanmakuEvent, DanmakuMethod, Site};

    #[test]
    fn map_empty_non_senddm_is_empty() {
        let ev = DanmakuEvent::new(Site::BiliLive, "1", DanmakuMethod::LiveDMServer, "", None);
        let out = map_event_to_ui(ev);
        assert!(out.is_empty());
    }

    #[test]
    fn map_dm_text_from_dms() {
        let mut ev = DanmakuEvent::new(Site::BiliLive, "1", DanmakuMethod::SendDM, "", None);
        ev.user = "U".to_string();
        ev.dms = Some(vec![DanmakuComment::text("hi")]);
        let out = map_event_to_ui(ev);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].user, "U");
        assert_eq!(out[0].text, "hi");
    }

    #[test]
    fn map_emoticon_without_text_keeps_placeholder() {
        let mut ev = DanmakuEvent::new(Site::BiliLive, "1", DanmakuMethod::SendDM, "", None);
        ev.user = "".to_string();
        ev.dms = Some(vec![DanmakuComment {
            text: "".to_string(),
            image_url: Some("https://example.com/a.png".to_string()),
            image_width: Some(64),
        }]);
        let out = map_event_to_ui(ev);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].user, "bili_live");
        assert_eq!(out[0].text, "[表情]");
        assert!(out[0].image_url.as_deref().unwrap().contains("example.com"));
        assert_eq!(out[0].image_width, Some(64));
    }

    #[test]
    fn fallback_to_ev_text_when_no_dms() {
        let mut ev = DanmakuEvent::new(Site::Douyu, "9", DanmakuMethod::SendDM, "hello", None);
        ev.user = "X".to_string();
        let out = map_event_to_ui(ev);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "hello");
        assert_eq!(out[0].user, "X");
    }

    #[test]
    fn dedupe_duplicate_comments_within_event() {
        let mut ev = DanmakuEvent::new(Site::BiliLive, "1", DanmakuMethod::SendDM, "", None);
        ev.user = "U".to_string();
        ev.dms = Some(vec![
            DanmakuComment::text("hi"),
            DanmakuComment::text("hi"),
            DanmakuComment::text("hi"),
        ]);
        let out = map_event_to_ui(ev);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "hi");
    }
}
