use std::sync::OnceLock;

use regex::Regex;
use strsim::jaro_winkler;

use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsSearchTerm};

const WEIGHT_TITLE: f64 = 0.30;
const WEIGHT_ARTIST: f64 = 0.30;
const WEIGHT_ALBUM: f64 = 0.10;
const WEIGHT_DURATION: f64 = 0.30;

pub fn compute_match_percentage(req: &LyricsSearchRequest, result: &LyricsSearchResult) -> u8 {
    let (local_has_meta, local_title, local_artist, local_album) = match &req.term {
        LyricsSearchTerm::Info {
            title,
            artist,
            album,
        } => {
            let t = title.trim();
            let a = artist.trim();
            (
                !t.is_empty() && !a.is_empty(),
                Some(t),
                Some(a),
                album.as_deref().map(|s| s.trim()),
            )
        }
        LyricsSearchTerm::Keyword { .. } => (false, None, None, None),
    };

    let remote_title = result
        .title
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let remote_artist = result
        .artist
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let remote_album = result
        .album
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    let remote_has_meta = remote_title.is_some();

    let score = if local_has_meta && remote_has_meta {
        let title_score = string_similarity(
            local_title.unwrap_or_default(),
            remote_title.unwrap_or_default(),
        );
        let artist_score = string_similarity(
            local_artist.unwrap_or_default(),
            remote_artist.unwrap_or_default(),
        );
        let album_score = string_similarity(
            local_album.unwrap_or_default(),
            remote_album.unwrap_or_default(),
        );
        let duration_score = duration_similarity_ms(req.duration_ms, result.duration_ms);

        (title_score * WEIGHT_TITLE)
            + (artist_score * WEIGHT_ARTIST)
            + (album_score * WEIGHT_ALBUM)
            + (duration_score * WEIGHT_DURATION)
    } else {
        let local_query = if local_has_meta {
            format!(
                "{} {}",
                local_title.unwrap_or_default(),
                local_artist.unwrap_or_default()
            )
        } else {
            req.term.description()
        };
        let remote_query = if remote_has_meta {
            format!(
                "{} {}",
                remote_title.unwrap_or_default(),
                remote_artist.unwrap_or_default()
            )
        } else {
            String::new()
        };

        let fp1 = fingerprint(&local_query);
        let fp2 = fingerprint(&remote_query);
        if fp1.is_empty() || fp2.is_empty() {
            0.0
        } else {
            string_similarity(&fp1, &fp2)
        }
    };

    (score.clamp(0.0, 1.0) * 100.0).round().clamp(0.0, 100.0) as u8
}

fn string_similarity(a: &str, b: &str) -> f64 {
    let a = a.trim().to_lowercase();
    let b = b.trim().to_lowercase();

    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    jaro_winkler(&a, &b).clamp(0.0, 1.0)
}

fn duration_similarity_ms(local: Option<u64>, remote: Option<u64>) -> f64 {
    let (Some(a), Some(b)) = (local, remote) else {
        return 0.0;
    };
    if a == 0 || b == 0 {
        return 0.0;
    }
    let diff = a.max(b) - a.min(b);
    const PERFECT_TOL_MS: u64 = 1_000;
    const MAX_TOL_MS: u64 = 10_000;
    if diff <= PERFECT_TOL_MS {
        return 1.0;
    }
    if diff >= MAX_TOL_MS {
        return 0.0;
    }
    1.0 - ((diff - PERFECT_TOL_MS) as f64) / ((MAX_TOL_MS - PERFECT_TOL_MS) as f64)
}

fn fingerprint(input: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[\p{P}\p{S}]").expect("fingerprint regex"));

    let lower = input.to_lowercase();
    let cleaned = re.replace_all(&lower, " ");
    let mut tokens: Vec<&str> = cleaned.split_whitespace().collect();
    tokens.sort_unstable();
    tokens.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::model::{LyricsSearchTerm, LyricsService};

    fn req_info(
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_ms: Option<u64>,
    ) -> LyricsSearchRequest {
        let mut req = LyricsSearchRequest::new(LyricsSearchTerm::Info {
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.map(|s| s.to_string()),
        });
        req.duration_ms = duration_ms;
        req
    }

    fn res(
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        duration_ms: Option<u64>,
    ) -> LyricsSearchResult {
        LyricsSearchResult {
            service: LyricsService::QQMusic,
            service_token: "t".to_string(),
            title: title.map(|s| s.to_string()),
            artist: artist.map(|s| s.to_string()),
            album: album.map(|s| s.to_string()),
            duration_ms,
            match_percentage: 0,
            quality: 0.0,
            matched: false,
            has_translation: false,
            has_inline_timetags: false,
            lyrics_original: "x".to_string(),
            lyrics_translation: None,
            debug: None,
        }
    }

    #[test]
    fn duration_similarity_curve_matches_betterlyrics_rules() {
        let req = req_info("t", "a", None, Some(300_000));
        let r_perfect = res(Some("t"), Some("a"), None, Some(299_500));
        let r_far = res(Some("t"), Some("a"), None, Some(200_000));

        let s1 = compute_match_percentage(&req, &r_perfect);
        let s2 = compute_match_percentage(&req, &r_far);
        assert!(s1 > s2);
    }

    #[test]
    fn fingerprint_fallback_produces_nonzero_for_similar_queries() {
        let req = LyricsSearchRequest::new(LyricsSearchTerm::Keyword {
            keyword: "Hello Adele".to_string(),
        });
        let r = res(Some("Hello"), Some("Adele"), None, None);
        let s = compute_match_percentage(&req, &r);
        assert!(s > 0);
    }
}
