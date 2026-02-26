use serde::{Deserialize, Serialize};

use crate::TtsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptStrategy {
    Inject,
    GuidePrefix,
}

impl PromptStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inject => "inject",
            Self::GuidePrefix => "guide_prefix",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTtsText {
    pub spoken_text: String,
    pub prompt_inject_text: String,
}

pub const END_OF_PROMPT: &str = "<|endofprompt|>";

/// A minimal, deterministic text frontend intended to be "good enough" without bringing
/// in `wetext`/`ttsfrd`. This is NOT meant to perfectly match CosyVoice python behavior.
fn normalize_basic(mut s: String) -> String {
    // Remove newlines and normalize whitespace.
    s = s.replace('\n', "").replace('\r', "");
    s = collapse_whitespace(&s);

    // Align some punctuation used in the upstream frontend.
    s = s.replace(" - ", "，");
    s = s.replace('.', "。");

    // Remove common bracketed asides, but do not touch the `<|...|>` markers.
    s = remove_bracketed_asides(&s);

    // Ensure Chinese text ends with sentence punctuation (best-effort).
    if contains_chinese(&s) && !s.is_empty() {
        let last = s.chars().last().unwrap();
        if !matches!(last, '。' | '！' | '？' | '!' | '?' | '…') {
            s.push('。');
        }
    }

    s.trim().to_string()
}

pub fn resolve_tts_text_basic(
    text: &str,
    prompt_text: &str,
    prompt_strategy: PromptStrategy,
    guide_sep: &str,
    text_frontend: bool,
) -> Result<ResolvedTtsText, TtsError> {
    let mut text = text.trim().to_string();
    if text.is_empty() {
        return Err(TtsError::InvalidArg("text is empty".into()));
    }

    let mut prompt_text = prompt_text.trim().to_string();

    if text_frontend {
        text = normalize_basic(text);
        if !prompt_text.is_empty() {
            prompt_text = normalize_basic(prompt_text);
        }
    }

    let (spoken_text, mut prompt_inject_text) = match prompt_strategy {
        PromptStrategy::Inject => (text, prompt_text),
        PromptStrategy::GuidePrefix => {
            // "guide_prefix" mode: prepend guide text to spoken text, but only inject END_OF_PROMPT.
            let guide = strip_endofprompt(&prompt_text);
            let spoken = if guide.is_empty() {
                text
            } else {
                format!("{}{}{}", guide, guide_sep, text)
            };
            (spoken, END_OF_PROMPT.to_string())
        }
    };

    if !prompt_inject_text.is_empty() && !prompt_inject_text.contains(END_OF_PROMPT) {
        prompt_inject_text.push_str(END_OF_PROMPT);
    }

    if prompt_inject_text.is_empty() && !spoken_text.contains(END_OF_PROMPT) {
        return Err(TtsError::InvalidArg(format!(
            "CosyVoice3 requires {END_OF_PROMPT} in prompt_text or text"
        )));
    }

    Ok(ResolvedTtsText {
        spoken_text,
        prompt_inject_text,
    })
}

fn strip_endofprompt(s: &str) -> String {
    s.replace(END_OF_PROMPT, "").trim().to_string()
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        let is_ws = ch.is_whitespace();
        if is_ws {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn contains_chinese(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

fn remove_bracketed_asides(s: &str) -> String {
    // Remove (...) and （...） and [...] and 【...】. Keep marker "<|endofprompt|>" intact.
    // This is intentionally simple and does not handle nesting robustly.
    let mut out = String::with_capacity(s.len());
    let mut depth_round = 0usize;
    let mut depth_square = 0usize;
    let mut depth_cjk_square = 0usize;
    let mut depth_cjk_round = 0usize;

    for ch in s.chars() {
        match ch {
            '(' => depth_round += 1,
            ')' => depth_round = depth_round.saturating_sub(1),
            '[' => depth_square += 1,
            ']' => depth_square = depth_square.saturating_sub(1),
            '【' => depth_cjk_square += 1,
            '】' => depth_cjk_square = depth_cjk_square.saturating_sub(1),
            '（' => depth_cjk_round += 1,
            '）' => depth_cjk_round = depth_cjk_round.saturating_sub(1),
            _ => {
                if depth_round == 0 && depth_square == 0 && depth_cjk_square == 0 && depth_cjk_round == 0 {
                    out.push(ch);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_appends_endofprompt_when_missing() {
        let r = resolve_tts_text_basic(
            "你好",
            "我是提示",
            PromptStrategy::Inject,
            "。 ",
            false,
        )
        .unwrap();
        assert_eq!(r.spoken_text, "你好");
        assert!(r.prompt_inject_text.ends_with(END_OF_PROMPT));
        assert!(r.prompt_inject_text.contains("我是提示"));
    }

    #[test]
    fn empty_prompt_requires_marker_in_spoken_text() {
        let err = resolve_tts_text_basic("你好", "", PromptStrategy::Inject, " ", false).unwrap_err();
        assert!(matches!(err, TtsError::InvalidArg(_)));
    }

    #[test]
    fn guide_prefix_prepends_guide_and_injects_marker_only() {
        let r = resolve_tts_text_basic(
            "看到码头就发马头",
            "我在抖音上老刷那种...<|endofprompt|>",
            PromptStrategy::GuidePrefix,
            "。 ",
            false,
        )
        .unwrap();
        assert!(r.spoken_text.starts_with("我在抖音上老刷那种..."));
        assert!(r.spoken_text.contains("。 "));
        assert_eq!(r.prompt_inject_text, END_OF_PROMPT);
    }

    #[test]
    fn basic_normalize_keeps_marker_and_adds_chinese_period() {
        let r = resolve_tts_text_basic(
            "你好",
            "<|endofprompt|>",
            PromptStrategy::Inject,
            " ",
            true,
        )
        .unwrap();
        // Spoken text becomes "你好。" under basic normalization.
        assert_eq!(r.spoken_text, "你好。");
        assert_eq!(r.prompt_inject_text, END_OF_PROMPT);
    }
}
