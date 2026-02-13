use serde_json::Value;

use crate::danmaku::model::Site;

use super::super::client::{LiveDirectoryClient, LiveDirectoryError};
use super::super::model::{LiveCategory, LiveRoomCard, LiveRoomList, LiveSubCategory};

fn make_input(room_id: &str) -> String {
    format!("huya:{room_id}")
}

fn is_html_like(s: &str) -> bool {
    let t = s.trim_start();
    t.starts_with("<!DOCTYPE") || t.starts_with("<html") || t.starts_with('<')
}

async fn get_value(
    client: &LiveDirectoryClient,
    url: &str,
    query: &[(String, String)],
) -> Result<Value, LiveDirectoryError> {
    // Huya endpoints will sometimes return non-JSON (HTML/empty) when intercepted.
    // Use a realistic UA + referer like dart_simple_live to reduce interception rate.
    const UA: &str = "HYSDK(Windows, 30000002)_APP(pc_exe&7060000&official)_SDK(trans&2.32.3.5646)";
    const UA_FALLBACK: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";
    const BASE: &str = "https://m.huya.com/";

    async fn do_req(
        client: &LiveDirectoryClient,
        url: &str,
        query: &[(String, String)],
        ua: &str,
    ) -> Result<Value, LiveDirectoryError> {
        let resp = client
            .http
            .get(url)
            .query(query)
            .header(reqwest::header::USER_AGENT, ua)
            .header(reqwest::header::REFERER, BASE)
            .header(reqwest::header::ORIGIN, BASE)
            .header(reqwest::header::ACCEPT, "application/json, text/plain, */*")
            .send()
            .await?
            .error_for_status()?;

        let mut text = resp.text().await?;
        // Strip UTF-8 BOM if present.
        if text.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]) {
            text = text.trim_start_matches('\u{feff}').to_string();
        }
        if text.trim().is_empty() {
            return Err(LiveDirectoryError::Parse(
                "huya response is empty".to_string(),
            ));
        }
        if is_html_like(&text) {
            let snippet = text.chars().take(200).collect::<String>();
            return Err(LiveDirectoryError::Parse(format!(
                "huya response is not json: {snippet}"
            )));
        }

        serde_json::from_str::<Value>(&text).map_err(|e| {
            let snippet = text.chars().take(200).collect::<String>();
            LiveDirectoryError::Parse(format!("huya json parse failed: {e}; body={snippet}"))
        })
    }

    match do_req(client, url, query, UA).await {
        Ok(v) => Ok(v),
        Err(e) => {
            // One retry with a common browser UA.
            match do_req(client, url, query, UA_FALLBACK).await {
                Ok(v) => Ok(v),
                Err(_) => Err(e),
            }
        }
    }
}

fn parse_gid(v: &Value) -> Option<String> {
    // Dart handles multiple shapes; do best-effort.
    if let Some(map) = v.as_object() {
        if let Some(val) = map.get("value").and_then(|x| x.as_str()) {
            return Some(val.split(',').next().unwrap_or("").trim().to_string());
        }
    }
    if let Some(n) = v.as_i64() {
        return Some(n.to_string());
    }
    if let Some(n) = v.as_f64() {
        return Some((n as i64).to_string());
    }
    v.as_str().map(|s| s.to_string())
}

pub async fn get_categories(
    client: &LiveDirectoryClient,
) -> Result<Vec<LiveCategory>, LiveDirectoryError> {
    let base = client
        .cfg
        .endpoints
        .huya_live_cdn_base
        .trim_end_matches('/');
    let url = format!("{base}/liveconfig/game/bussLive");

    let mut cats = vec![
        LiveCategory {
            id: "1".to_string(),
            name: "网游".to_string(),
            children: vec![],
        },
        LiveCategory {
            id: "2".to_string(),
            name: "单机".to_string(),
            children: vec![],
        },
        LiveCategory {
            id: "8".to_string(),
            name: "娱乐".to_string(),
            children: vec![],
        },
        LiveCategory {
            id: "3".to_string(),
            name: "手游".to_string(),
            children: vec![],
        },
    ];

    let mut ok_any = false;
    let mut last_err: Option<LiveDirectoryError> = None;
    for c in &mut cats {
        let v = match get_value(client, &url, &[("bussType".to_string(), c.id.clone())]).await {
            Ok(v) => {
                ok_any = true;
                v
            }
            Err(e) => {
                // Huya categories endpoint is occasionally flaky/intercepted; keep partial results.
                last_err = Some(e);
                continue;
            }
        };
        let arr = v
            .pointer("/data")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let mut subs = Vec::new();
        for x in arr {
            let gid = x.get("gid").and_then(parse_gid).unwrap_or_default();
            let name = x
                .get("gameFullName")
                .and_then(|y| y.as_str())
                .unwrap_or("")
                .to_string();
            if gid.is_empty() || name.is_empty() {
                continue;
            }
            subs.push(LiveSubCategory {
                id: gid.clone(),
                parent_id: c.id.clone(),
                name,
                pic: Some(format!(
                    "https://huyaimg.msstatic.com/cdnimage/game/{gid}-MS.jpg"
                )),
            });
        }
        c.children.extend(subs);
    }

    if !ok_any {
        return Err(last_err
            .unwrap_or_else(|| LiveDirectoryError::Parse("huya categories failed".to_string())));
    }
    Ok(cats)
}

pub async fn get_recommend_rooms(
    client: &LiveDirectoryClient,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    let base = client.cfg.endpoints.huya_base.trim_end_matches('/');
    let url = format!("{base}/cache.php");
    let v = get_value(
        client,
        &url,
        &[
            ("m".to_string(), "LiveList".to_string()),
            ("do".to_string(), "getLiveListByPage".to_string()),
            ("tagAll".to_string(), "0".to_string()),
            ("page".to_string(), page.max(1).to_string()),
        ],
    )
    .await?;

    let datas = v
        .pointer("/data/datas")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for x in datas {
        let rid = x
            .get("profileRoom")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .or_else(|| {
                x.get("profileRoom")
                    .and_then(|y| y.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let mut cover = x
            .get("screenshot")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        if !cover.is_empty() && !cover.contains('?') {
            cover.push_str("?x-oss-process=style/w338_h190&");
        }
        let intro = x.get("introduction").and_then(|y| y.as_str()).unwrap_or("");
        let room_name = x.get("roomName").and_then(|y| y.as_str()).unwrap_or("");
        let title = if intro.trim().is_empty() {
            room_name.to_string()
        } else {
            intro.to_string()
        };
        let user_name = x
            .get("nick")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let online = x
            .get("totalCount")
            .and_then(|y| y.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .or_else(|| x.get("totalCount").and_then(|y| y.as_i64()));

        items.push(LiveRoomCard {
            site: Site::Huya,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover: (!cover.trim().is_empty()).then_some(cover),
            user_name,
            online,
        });
    }

    let has_more = v
        .pointer("/data/page")
        .and_then(|x| x.as_i64())
        .zip(v.pointer("/data/totalPage").and_then(|x| x.as_i64()))
        .map(|(p, t)| p < t)
        .unwrap_or(!items.is_empty());

    Ok(LiveRoomList { has_more, items })
}

pub async fn get_category_rooms(
    client: &LiveDirectoryClient,
    category_id: &str,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    let cid = category_id.trim();
    if cid.is_empty() {
        return Err(LiveDirectoryError::InvalidInput(
            "category_id is empty".to_string(),
        ));
    }
    let base = client.cfg.endpoints.huya_base.trim_end_matches('/');
    let url = format!("{base}/cache.php");
    let v = get_value(
        client,
        &url,
        &[
            ("m".to_string(), "LiveList".to_string()),
            ("do".to_string(), "getLiveListByPage".to_string()),
            ("tagAll".to_string(), "0".to_string()),
            ("gameId".to_string(), cid.to_string()),
            ("page".to_string(), page.max(1).to_string()),
        ],
    )
    .await?;
    // Response shape matches recommend.
    let datas = v
        .pointer("/data/datas")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for x in datas {
        let rid = x
            .get("profileRoom")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .or_else(|| {
                x.get("profileRoom")
                    .and_then(|y| y.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let mut cover = x
            .get("screenshot")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        if !cover.is_empty() && !cover.contains('?') {
            cover.push_str("?x-oss-process=style/w338_h190&");
        }
        let intro = x.get("introduction").and_then(|y| y.as_str()).unwrap_or("");
        let room_name = x.get("roomName").and_then(|y| y.as_str()).unwrap_or("");
        let title = if intro.trim().is_empty() {
            room_name.to_string()
        } else {
            intro.to_string()
        };
        let user_name = x
            .get("nick")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let online = x
            .get("totalCount")
            .and_then(|y| y.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .or_else(|| x.get("totalCount").and_then(|y| y.as_i64()));
        items.push(LiveRoomCard {
            site: Site::Huya,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover: (!cover.trim().is_empty()).then_some(cover),
            user_name,
            online,
        });
    }
    let has_more = v
        .pointer("/data/page")
        .and_then(|x| x.as_i64())
        .zip(v.pointer("/data/totalPage").and_then(|x| x.as_i64()))
        .map(|(p, t)| p < t)
        .unwrap_or(!items.is_empty());
    Ok(LiveRoomList { has_more, items })
}

pub async fn search_rooms(
    client: &LiveDirectoryClient,
    keyword: &str,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    // Huya search endpoints differ and are not part of the plan; match dart_simple_live.
    let kw = keyword.trim();
    if kw.is_empty() {
        return Err(LiveDirectoryError::InvalidInput(
            "keyword is empty".to_string(),
        ));
    }
    let base = client.cfg.endpoints.huya_search_base.trim_end_matches('/');
    let url = format!("{base}/");
    let v = get_value(
        client,
        &url,
        &[
            ("m".to_string(), "Search".to_string()),
            ("do".to_string(), "getSearchContent".to_string()),
            ("q".to_string(), kw.to_string()),
            ("uid".to_string(), "0".to_string()),
            ("v".to_string(), "4".to_string()),
            ("typ".to_string(), "-5".to_string()),
            ("livestate".to_string(), "0".to_string()),
            ("rows".to_string(), "20".to_string()),
            ("start".to_string(), ((page.max(1) - 1) * 20).to_string()),
        ],
    )
    .await?;

    let list = v
        .pointer("/response/3/docs")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let num_found = v
        .pointer("/response/3/numFound")
        .and_then(|x| x.as_i64())
        .unwrap_or(list.len() as i64);
    let mut items = Vec::new();
    for x in list {
        let rid = x
            .get("room_id")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        if rid.is_empty() {
            continue;
        }
        let mut title = x
            .get("game_introduction")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        if title.trim().is_empty() {
            title = x
                .get("game_roomName")
                .and_then(|y| y.as_str())
                .unwrap_or("")
                .to_string();
        }
        let mut cover = x
            .get("game_screenshot")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        if !cover.is_empty() && !cover.contains('?') {
            cover.push_str("?x-oss-process=style/w338_h190&");
        }
        let user_name = x
            .get("game_nick")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let online = x.get("game_total_count").and_then(|y| y.as_i64());
        items.push(LiveRoomCard {
            site: Site::Huya,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover: (!cover.trim().is_empty()).then_some(cover),
            user_name,
            online,
        });
    }
    Ok(LiveRoomList {
        has_more: num_found > (page.max(1) as i64) * 20,
        items,
    })
}
