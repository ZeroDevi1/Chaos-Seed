use std::time::Duration;

use futures_util::stream::{FuturesUnordered, StreamExt};

use crate::lyrics::error::LyricsError;
use crate::lyrics::model::{LyricsSearchOptions, LyricsSearchRequest, LyricsSearchResult, LyricsService};
use crate::lyrics::providers::Provider;
use crate::lyrics::quality;

pub async fn search(
    req: &LyricsSearchRequest,
    opt: LyricsSearchOptions,
) -> Result<Vec<LyricsSearchResult>, LyricsError> {
    crate::tls::ensure_rustls_provider();
    let http = reqwest::Client::builder()
        .user_agent("chaos-seed/0.1")
        .build()?;
    search_with_http(&http, req, opt).await
}

pub async fn search_with_http(
    http: &reqwest::Client,
    req: &LyricsSearchRequest,
    opt: LyricsSearchOptions,
) -> Result<Vec<LyricsSearchResult>, LyricsError> {
    let timeout = Duration::from_millis(opt.timeout_ms.max(1));

    let providers = build_providers(&opt.services);
    let mut provider_tasks = FuturesUnordered::new();

    for p in providers {
        let http = http.clone();
        let req = req.clone();
        let timeout = timeout;
        provider_tasks.push(async move { search_one_provider(&http, p, &req, timeout).await });
    }

    let mut results: Vec<LyricsSearchResult> = Vec::new();
    while let Some(r) = provider_tasks.next().await {
        match r {
            Ok(mut items) => results.append(&mut items),
            Err(_) => {
                // Provider failure is tolerated: other providers may still succeed.
            }
        }
    }

    // Compute match + quality.
    for item in &mut results {
        item.matched = quality::is_matched(item, req);
        item.quality = quality::compute_quality(item, req);
    }

    if opt.strict_match {
        results.retain(|r| r.matched);
    }

    let order = service_order_index(&opt.services);
    results.sort_by(|a, b| {
        let qa = a.quality;
        let qb = b.quality;
        let by_q = qb
            .partial_cmp(&qa)
            .unwrap_or(std::cmp::Ordering::Equal);
        if by_q != std::cmp::Ordering::Equal {
            return by_q;
        }
        let ia = order.get(&a.service).copied().unwrap_or(usize::MAX);
        let ib = order.get(&b.service).copied().unwrap_or(usize::MAX);
        ia.cmp(&ib)
    });

    results.truncate(req.limit.max(1));
    Ok(results)
}

fn build_providers(services: &[LyricsService]) -> Vec<Provider> {
    services
        .iter()
        .copied()
        .map(|s| match s {
            LyricsService::Netease => Provider::Netease(Default::default()),
            LyricsService::QQMusic => Provider::QQ(Default::default()),
            LyricsService::Kugou => Provider::Kugou(Default::default()),
            LyricsService::Gecimi => Provider::Gecimi(Default::default()),
            LyricsService::Syair => Provider::Syair(Default::default()),
        })
        .collect()
}

fn service_order_index(
    services: &[LyricsService],
) -> std::collections::HashMap<LyricsService, usize> {
    let mut m = std::collections::HashMap::new();
    for (i, s) in services.iter().copied().enumerate() {
        m.insert(s, i);
    }
    m
}

async fn search_one_provider(
    http: &reqwest::Client,
    provider: Provider,
    req: &LyricsSearchRequest,
    timeout: Duration,
) -> Result<Vec<LyricsSearchResult>, LyricsError> {
    let tokens = match tokio::time::timeout(timeout, provider.search(http, req, timeout)).await {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err(LyricsError::Parse("provider search timeout".to_string())),
    };

    let mut fetch_tasks = FuturesUnordered::new();
    for token in tokens.into_iter().take(req.limit.max(1)) {
        let http = http.clone();
        let provider = provider.clone();
        let req = req.clone();
        fetch_tasks.push(async move {
            match tokio::time::timeout(timeout, provider.fetch(&http, token, &req, timeout)).await {
                Ok(Ok(v)) => Ok(v),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(LyricsError::Parse("provider fetch timeout".to_string())),
            }
        });
    }

    let mut out = Vec::new();
    while let Some(r) = fetch_tasks.next().await {
        if let Ok(item) = r {
            out.push(item);
        }
    }
    Ok(out)
}
