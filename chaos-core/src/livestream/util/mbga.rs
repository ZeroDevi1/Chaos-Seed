#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BiliCdn {
    Mirror = 0,
    Cache = 1,
    Mcdn = 2,
    Pcdn = 3,
}

fn cdn_level_for_host(host: &str) -> BiliCdn {
    let h = host.to_ascii_lowercase();
    if h.contains(".mcdn.bilivideo.cn") {
        BiliCdn::Mcdn
    } else if h.contains(".szbdyd.com") {
        BiliCdn::Pcdn
    } else if h.contains("bilivideo.com") && h.starts_with("up") {
        BiliCdn::Mirror
    } else {
        BiliCdn::Cache
    }
}

pub fn cdn_level(url: &str) -> i32 {
    match url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| cdn_level_for_host(h)))
        .unwrap_or(BiliCdn::Mcdn)
    {
        BiliCdn::Mirror => 0,
        BiliCdn::Cache => 1,
        BiliCdn::Mcdn => 2,
        BiliCdn::Pcdn => 3,
    }
}

/// Sort and de-duplicate urls by CDN preference (ported from IINA+'s MBGA.update).
pub fn sort_urls(urls: &[String]) -> Vec<String> {
    let mut uniq = std::collections::BTreeSet::<String>::new();
    for u in urls {
        uniq.insert(u.clone());
    }
    let mut out: Vec<String> = uniq.into_iter().collect();
    out.sort_by_key(|u| cdn_level(u));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_beats_mcdn() {
        let urls = vec![
            "https://foo.mcdn.bilivideo.cn/live-bvc/xx".to_string(),
            "https://up-gotcha.bilivideo.com/live-bvc/xx".to_string(),
        ];
        let sorted = sort_urls(&urls);
        assert_eq!(sorted[0], "https://up-gotcha.bilivideo.com/live-bvc/xx");
        assert_eq!(sorted[1], "https://foo.mcdn.bilivideo.cn/live-bvc/xx");
    }
}
