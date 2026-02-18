use super::BiliError;

pub fn select_page_indices(total_pages: usize, select_page: &str) -> Result<Vec<usize>, BiliError> {
    if total_pages == 0 {
        return Ok(vec![]);
    }

    let raw = select_page.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("all") {
        return Ok((0..total_pages).collect());
    }

    let mut out: Vec<usize> = Vec::new();

    for token in raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if token.eq_ignore_ascii_case("all") {
            for i in 0..total_pages {
                if !out.contains(&i) {
                    out.push(i);
                }
            }
            continue;
        }
        if token.eq_ignore_ascii_case("last") || token.eq_ignore_ascii_case("latest") {
            let i = total_pages - 1;
            if !out.contains(&i) {
                out.push(i);
            }
            continue;
        }
        if let Some((a, b)) = token.split_once('-') {
            let a = a.trim().parse::<isize>().ok();
            let b = b.trim().parse::<isize>().ok();
            let (Some(a), Some(b)) = (a, b) else { continue };
            let start = a.min(b);
            let end = a.max(b);
            for n in start..=end {
                if n <= 0 {
                    continue;
                }
                let idx = (n as usize).saturating_sub(1);
                if idx < total_pages && !out.contains(&idx) {
                    out.push(idx);
                }
            }
            continue;
        }

        if let Ok(n) = token.parse::<isize>() {
            if n > 0 {
                let idx = (n as usize).saturating_sub(1);
                if idx < total_pages && !out.contains(&idx) {
                    out.push(idx);
                }
            }
        }
    }

    out.sort_unstable();
    Ok(out)
}

