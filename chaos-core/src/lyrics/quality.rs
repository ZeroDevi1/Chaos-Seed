use crate::lyrics::model::{LyricsSearchRequest, LyricsSearchResult, LyricsSearchTerm};

const TRANSLATION_BONUS: f64 = 0.1;
const INLINE_TIMETAG_BONUS: f64 = 0.1;
const MATCHED_ARTIST_FACTOR: f64 = 1.3;
const MATCHED_TITLE_FACTOR: f64 = 1.5;
const NO_ARTIST_FACTOR: f64 = 0.8;
const NO_TITLE_FACTOR: f64 = 0.8;
const NO_DURATION_FACTOR: f64 = 0.8;
const MINIMAL_DURATION_QUALITY: f64 = 0.5;
const QUALITY_MIX_BOUND: f64 = 1.05;

pub fn is_matched(result: &LyricsSearchResult, req: &LyricsSearchRequest) -> bool {
    let Some(title) = result.title.as_deref() else {
        return false;
    };
    let Some(artist) = result.artist.as_deref() else {
        return false;
    };
    match &req.term {
        LyricsSearchTerm::Info {
            title: st,
            artist: sa,
            ..
        } => is_case_insensitive_similar(title, st) && is_case_insensitive_similar(artist, sa),
        LyricsSearchTerm::Keyword { keyword } => {
            is_case_insensitive_similar(title, keyword)
                && is_case_insensitive_similar(artist, keyword)
        }
    }
}

pub fn compute_quality(result: &LyricsSearchResult, req: &LyricsSearchRequest) -> f64 {
    let artist_quality = artist_quality(result, req);
    let title_quality = title_quality(result, req);
    let duration_quality = duration_quality(result, req);

    let mix = (QUALITY_MIX_BOUND - artist_quality)
        * (QUALITY_MIX_BOUND - title_quality)
        * (QUALITY_MIX_BOUND - duration_quality);
    let mut quality = 1.0 - mix.cbrt();

    if result.has_translation {
        quality += TRANSLATION_BONUS;
    }
    if result.has_inline_timetags {
        quality += INLINE_TIMETAG_BONUS;
    }

    if quality.is_finite() { quality } else { 0.0 }
}

pub fn is_case_insensitive_similar(a: &str, b: &str) -> bool {
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    a.contains(&b) || b.contains(&a)
}

fn artist_quality(result: &LyricsSearchResult, req: &LyricsSearchRequest) -> f64 {
    let Some(artist) = result.artist.as_deref() else {
        return NO_ARTIST_FACTOR;
    };
    match &req.term {
        LyricsSearchTerm::Info { artist: sa, .. } => {
            if artist == sa {
                return MATCHED_ARTIST_FACTOR;
            }
            similarity(artist, sa)
        }
        LyricsSearchTerm::Keyword { keyword } => {
            if keyword.contains(artist) {
                return MATCHED_ARTIST_FACTOR;
            }
            similarity_in(artist, keyword)
        }
    }
}

fn title_quality(result: &LyricsSearchResult, req: &LyricsSearchRequest) -> f64 {
    let Some(title) = result.title.as_deref() else {
        return NO_TITLE_FACTOR;
    };
    match &req.term {
        LyricsSearchTerm::Info { title: st, .. } => {
            if title == st {
                return MATCHED_TITLE_FACTOR;
            }
            similarity(title, st)
        }
        LyricsSearchTerm::Keyword { keyword } => {
            if keyword.contains(title) {
                return MATCHED_TITLE_FACTOR;
            }
            similarity_in(title, keyword)
        }
    }
}

fn duration_quality(result: &LyricsSearchResult, req: &LyricsSearchRequest) -> f64 {
    let (Some(d1), Some(d2)) = (result.duration_ms, req.duration_ms) else {
        return NO_DURATION_FACTOR;
    };
    // Quality curve matches LyricsKit's "dt < 10s" logic.
    let dt_sec = ((d1 as i64 - d2 as i64).abs() as f64) / 1000.0;
    if dt_sec >= 10.0 {
        return MINIMAL_DURATION_QUALITY;
    }
    1.0 - (1.0 - (dt_sec / 10.0)).powi(2) * (1.0 - MINIMAL_DURATION_QUALITY)
}

fn similarity(s1: &str, s2: &str) -> f64 {
    let len = s1.chars().count().min(s2.chars().count());
    if len == 0 {
        return 0.0;
    }
    let d1 = distance(s1, s2, 1, 0, 1);
    let d2 = distance(s1, s2, 1, 1, 0);
    let diff = d1.min(d2);
    (len.saturating_sub(diff) as f64) / (len as f64)
}

fn similarity_in(s1: &str, s2: &str) -> f64 {
    let len = s1.chars().count().max(s2.chars().count());
    if len == 0 {
        return 1.0;
    }
    let diff = distance(s1, s2, 1, 0, 1);
    (len.saturating_sub(diff) as f64) / (len as f64)
}

/// A simple Levenshtein distance with custom costs, working on Unicode scalar values.
fn distance(s1: &str, s2: &str, sub_cost: usize, ins_cost: usize, del_cost: usize) -> usize {
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    if b.is_empty() {
        return a.len() * del_cost;
    }
    let mut d: Vec<usize> = (0..=b.len()).map(|i| i * ins_cost).collect();
    for (i, c1) in a.iter().enumerate() {
        let mut prev = d[0];
        d[0] = (i + 1) * del_cost;
        for (j, c2) in b.iter().enumerate() {
            let tmp = d[j + 1];
            if c1 == c2 {
                d[j + 1] = prev;
            } else {
                let sub = prev + sub_cost;
                let ins = d[j] + ins_cost;
                let del = tmp + del_cost;
                d[j + 1] = sub.min(ins).min(del);
            }
            prev = tmp;
        }
    }
    d[b.len()]
}
