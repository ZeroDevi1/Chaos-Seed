use serde::{Deserialize, Serialize};

use crate::tts::TtsError;

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
            // "guide_prefix" mode:
            // - We keep `text` as the actual spoken text
            // - We prepend the guide into the *prompt* side, and ensure `<|endofprompt|>` sits
            //   right at the boundary between (guide prompt) and (spoken text).
            //
            // This matches CosyVoice3LM's behavior: it concatenates `prompt_text_tokens + text_tokens`.
            // If we only inject `<|endofprompt|>` and move the guide into `spoken_text`, the marker
            // would incorrectly appear *before* the guide, which can badly degrade generation.
            let guide = strip_endofprompt(&prompt_text);
            let prompt = if guide.is_empty() {
                END_OF_PROMPT.to_string()
            } else {
                format!("{}{}{}", guide, guide_sep, END_OF_PROMPT)
            };
            (text, prompt)
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

/// 计算 `guide_prefix` 模式下「需要从生成音频前面裁掉的比例」。（历史兼容）
///
/// 说明：旧实现曾把 guide 直接拼到 `spoken_text` 前面，因此需要按 token 占比裁掉音频前缀。
/// 目前 `guide_prefix` 已改为把 guide 放到 `prompt_inject_text`（不应被“读出来”），因此通常返回 None。
///
/// 这里用 tokenizer 的 token 序列做一个“后缀匹配”：
/// - full：guide + sep + text
/// - tail：text（同样经过 basic normalize）
/// 然后估算 guide 前缀 token 占比，供上层把音频前缀按比例裁剪。
pub fn compute_guide_prefix_ratio_tokens(
    tokenizer: &tokenizers::Tokenizer,
    add_special_tokens: bool,
    text: &str,
    prompt_text: &str,
    guide_sep: &str,
    text_frontend: bool,
) -> Result<Option<f32>, TtsError> {
    let full = resolve_tts_text_basic(
        text,
        prompt_text,
        PromptStrategy::GuidePrefix,
        guide_sep,
        text_frontend,
    )?;
    let tail = resolve_tts_text_basic(
        text,
        END_OF_PROMPT,
        PromptStrategy::Inject,
        guide_sep,
        text_frontend,
    )?;

    // 没有 guide（或 guide 为空）时无需裁剪。
    if full.spoken_text == tail.spoken_text {
        return Ok(None);
    }

    let full_ids = tokenizer
        .encode(full.spoken_text, add_special_tokens)
        .map_err(|e| TtsError::Tokenizer(format!("encode full spoken_text failed: {e}")))?
        .get_ids()
        .to_vec();
    let tail_ids = tokenizer
        .encode(tail.spoken_text, add_special_tokens)
        .map_err(|e| TtsError::Tokenizer(format!("encode tail spoken_text failed: {e}")))?
        .get_ids()
        .to_vec();

    if full_ids.is_empty() {
        return Ok(None);
    }

    let full_len = full_ids.len();
    let tail_len = tail_ids.len();

    // 优先走 “tail 完全匹配 full 的后缀” 的快路径。
    let suffix_match = if tail_len > 0
        && tail_len <= full_len
        && full_ids[full_len - tail_len..] == tail_ids[..]
    {
        tail_len
    } else {
        // 否则做一个最长“后缀匹配”（允许 tokenizer 在拼接边界处有少量差异）。
        let max = full_len.min(tail_len);
        let mut best = 0usize;
        for k in (1..=max).rev() {
            if full_ids[full_len - k..] == tail_ids[tail_len - k..] {
                best = k;
                break;
            }
        }
        best
    };

    let prefix_len = if suffix_match > 0 {
        full_len.saturating_sub(suffix_match)
    } else {
        // 兜底：用长度差估算（不保证严格正确，但比 0 好）。
        full_len.saturating_sub(tail_len)
    };

    if prefix_len == 0 {
        return Ok(None);
    }

    let r = (prefix_len as f32) / (full_len as f32);
    if !r.is_finite() || r <= 0.0 {
        return Ok(None);
    }
    Ok(Some(r.clamp(0.0, 1.0)))
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
                if depth_round == 0
                    && depth_square == 0
                    && depth_cjk_square == 0
                    && depth_cjk_round == 0
                {
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
        let r = resolve_tts_text_basic("你好", "我是提示", PromptStrategy::Inject, "。 ", false)
            .unwrap();
        assert_eq!(r.spoken_text, "你好");
        assert!(r.prompt_inject_text.ends_with(END_OF_PROMPT));
        assert!(r.prompt_inject_text.contains("我是提示"));
    }

    #[test]
    fn empty_prompt_requires_marker_in_spoken_text() {
        let err =
            resolve_tts_text_basic("你好", "", PromptStrategy::Inject, " ", false).unwrap_err();
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
        let r =
            resolve_tts_text_basic("你好", "<|endofprompt|>", PromptStrategy::Inject, " ", true)
                .unwrap();
        assert_eq!(r.spoken_text, "你好。");
        assert_eq!(r.prompt_inject_text, END_OF_PROMPT);
    }
}
