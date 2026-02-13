use serde_json::Value;

use crate::danmaku::model::Site;

use super::super::client::{LiveDirectoryClient, LiveDirectoryError};
use super::super::model::{LiveCategory, LiveRoomCard, LiveRoomList, LiveSubCategory};

fn parse_bili_code(err: &LiveDirectoryError) -> Option<i64> {
    let LiveDirectoryError::Parse(s) = err else {
        return None;
    };
    let key = "code=";
    let idx = s.find(key)?;
    let rest = &s[(idx + key.len())..];
    let end = rest.find(')')?;
    rest[..end].parse::<i64>().ok()
}

fn is_retryable_bili_code(code: i64) -> bool {
    // -352 / -412 are commonly seen for request interception / signature/device checks.
    // Refreshing buvid + wbi keys may help for the next attempt.
    matches!(code, -352 | -412)
}

fn abs_cover(url: &str) -> Option<String> {
    let u = url.trim();
    if u.is_empty() {
        return None;
    }
    if u.starts_with("//") {
        return Some(format!("https:{u}"));
    }
    Some(u.to_string())
}

fn make_input(room_id: &str) -> String {
    format!("bilibili:{room_id}")
}

async fn get_json(
    client: &LiveDirectoryClient,
    url: &str,
    query: &[(String, String)],
) -> Result<Value, LiveDirectoryError> {
    let mut req = client
        .http
        .get(url)
        .query(query)
        .header(reqwest::header::REFERER, "https://live.bilibili.com/")
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
        );

    if let Some(cookie) = super::super::util::bili_wbi::BiliWbi::ensure_buvid_cookie(client).await {
        req = req.header(reqwest::header::COOKIE, cookie);
    }

    let resp = req.send().await?.error_for_status()?;
    let v = resp.json::<Value>().await?;
    // Bilibili APIs usually return HTTP 200 even when business error happens.
    if let Some(code) = v.get("code").and_then(|x| x.as_i64()) {
        if code != 0 {
            let msg = v
                .get("message")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown error");
            return Err(LiveDirectoryError::Parse(format!(
                "bilibili api error (code={code}): {msg}"
            )));
        }
    }
    Ok(v)
}

pub async fn get_categories(
    client: &LiveDirectoryClient,
) -> Result<Vec<LiveCategory>, LiveDirectoryError> {
    let base = client
        .cfg
        .endpoints
        .bili_live_api_base
        .trim_end_matches('/');
    let url = format!("{base}/room/v1/Area/getList");
    let v = get_json(
        client,
        &url,
        &[
            ("need_entrance".to_string(), "1".to_string()),
            ("parent_id".to_string(), "0".to_string()),
        ],
    )
    .await?;

    let arr = v
        .pointer("/data")
        .and_then(|x| x.as_array())
        .ok_or_else(|| LiveDirectoryError::Parse("bili categories: missing data".to_string()))?;

    let mut out = Vec::new();
    for item in arr {
        let id = item
            .get("id")
            .and_then(|x| x.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        let name = item
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let list = item
            .get("list")
            .and_then(|x| x.as_array())
            .cloned()
            .unwrap_or_default();
        let mut subs = Vec::new();
        for sub in list {
            let sid = sub
                .get("id")
                .and_then(|x| x.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            let sname = sub
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let parent_id = sub
                .get("parent_id")
                .and_then(|x| x.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            let pic = sub
                .get("pic")
                .and_then(|x| x.as_str())
                .and_then(|s| (!s.trim().is_empty()).then_some(s))
                .and_then(abs_cover)
                .map(|u| {
                    if u.contains('@') {
                        u
                    } else {
                        format!("{u}@100w.png")
                    }
                });
            if sid.is_empty() || sname.is_empty() {
                continue;
            }
            subs.push(LiveSubCategory {
                id: sid,
                parent_id,
                name: sname,
                pic,
            });
        }
        if id.is_empty() || name.is_empty() {
            continue;
        }
        out.push(LiveCategory {
            id,
            name,
            children: subs,
        });
    }
    Ok(out)
}

pub async fn get_recommend_rooms(
    client: &LiveDirectoryClient,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    let base = client
        .cfg
        .endpoints
        .bili_live_api_base
        .trim_end_matches('/');
    let url = format!("{base}/xlive/web-interface/v1/second/getListByArea");

    let page_s = page.max(1).to_string();
    let base_params = vec![
        ("platform".to_string(), "web".to_string()),
        ("sort".to_string(), "online".to_string()),
        ("page_size".to_string(), "30".to_string()),
        ("page".to_string(), page_s),
    ];

    // Retry once on common interception codes by refreshing buvid + wbi keys.
    let mut last_err: Option<LiveDirectoryError> = None;
    let v = 'outer: loop {
        for attempt in 0..2 {
            let now_s = (client.cfg.env.now_s)();
            let keys = {
                let cached = client
                    .bili_wbi
                    .lock()
                    .expect("bili_wbi mutex")
                    .cached_keys();
                if let Some(k) = cached {
                    k
                } else {
                    let fetched = super::super::util::bili_wbi::BiliWbi::fetch_keys(client).await?;
                    client
                        .bili_wbi
                        .lock()
                        .expect("bili_wbi mutex")
                        .set_keys(fetched.clone());
                    fetched
                }
            };
            let mixin =
                super::super::util::bili_wbi::BiliWbi::mixin_key(&(keys.img_key + &keys.sub_key))?;
            let signed =
                super::super::util::bili_wbi::BiliWbi::sign_query(&base_params, &mixin, now_s);

            match get_json(client, &url, &signed).await {
                Ok(v) => break 'outer v,
                Err(e) => {
                    let retryable = parse_bili_code(&e).is_some_and(is_retryable_bili_code);
                    last_err = Some(e);
                    if attempt == 0 && retryable {
                        if let Ok(mut g) = client.bili_wbi.lock() {
                            g.clear_keys();
                            g.clear_access_id();
                            g.clear_buvid();
                        }
                        continue;
                    }
                    break;
                }
            }
        }
        return Err(last_err
            .unwrap_or_else(|| LiveDirectoryError::Parse("bilibili request failed".to_string())));
    };

    let list = v
        .pointer("/data/list")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for x in list {
        let rid = x
            .get("roomid")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let title = x
            .get("title")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        // Force jpg (dart_simple_live does "@400w.jpg") to avoid WebP decode issues on Windows.
        let cover = x
            .get("cover")
            .and_then(|y| y.as_str())
            .and_then(abs_cover)
            .map(|u| {
                if u.contains('@') {
                    u
                } else {
                    format!("{u}@400w.jpg")
                }
            });
        let user_name = x
            .get("uname")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let online = x.get("online").and_then(|y| y.as_i64());
        items.push(LiveRoomCard {
            site: Site::BiliLive,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover,
            user_name,
            online,
        });
    }

    Ok(LiveRoomList {
        has_more: !items.is_empty(),
        items,
    })
}

pub async fn get_category_rooms(
    client: &LiveDirectoryClient,
    parent_id: Option<&str>,
    category_id: &str,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    let pid = parent_id.unwrap_or("").trim();
    if pid.is_empty() {
        return Err(LiveDirectoryError::InvalidInput(
            "missing parent_id".to_string(),
        ));
    }
    let cid = category_id.trim();
    if cid.is_empty() {
        return Err(LiveDirectoryError::InvalidInput(
            "missing category_id".to_string(),
        ));
    }

    let base = client
        .cfg
        .endpoints
        .bili_live_api_base
        .trim_end_matches('/');
    let url = format!("{base}/xlive/web-interface/v1/second/getList");

    let mut last_err: Option<LiveDirectoryError> = None;
    let v = 'outer: loop {
        for attempt in 0..2 {
            let access_id = {
                let cached = client
                    .bili_wbi
                    .lock()
                    .expect("bili_wbi mutex")
                    .cached_access_id();
                if let Some(v) = cached {
                    v
                } else {
                    let fetched =
                        super::super::util::bili_wbi::BiliWbi::fetch_access_id(client).await?;
                    client
                        .bili_wbi
                        .lock()
                        .expect("bili_wbi mutex")
                        .set_access_id(fetched.clone());
                    fetched
                }
            };

            let params = vec![
                ("platform".to_string(), "web".to_string()),
                ("parent_area_id".to_string(), pid.to_string()),
                ("area_id".to_string(), cid.to_string()),
                ("sort_type".to_string(), "".to_string()),
                ("page".to_string(), page.max(1).to_string()),
                ("w_webid".to_string(), access_id),
            ];

            let now_s = (client.cfg.env.now_s)();
            let keys = {
                let cached = client
                    .bili_wbi
                    .lock()
                    .expect("bili_wbi mutex")
                    .cached_keys();
                if let Some(k) = cached {
                    k
                } else {
                    let fetched = super::super::util::bili_wbi::BiliWbi::fetch_keys(client).await?;
                    client
                        .bili_wbi
                        .lock()
                        .expect("bili_wbi mutex")
                        .set_keys(fetched.clone());
                    fetched
                }
            };
            let mixin =
                super::super::util::bili_wbi::BiliWbi::mixin_key(&(keys.img_key + &keys.sub_key))?;
            let signed = super::super::util::bili_wbi::BiliWbi::sign_query(&params, &mixin, now_s);

            match get_json(client, &url, &signed).await {
                Ok(v) => break 'outer v,
                Err(e) => {
                    let retryable = parse_bili_code(&e).is_some_and(is_retryable_bili_code);
                    last_err = Some(e);
                    if attempt == 0 && retryable {
                        if let Ok(mut g) = client.bili_wbi.lock() {
                            g.clear_keys();
                            g.clear_access_id();
                            g.clear_buvid();
                        }
                        continue;
                    }
                    break;
                }
            }
        }
        return Err(last_err
            .unwrap_or_else(|| LiveDirectoryError::Parse("bilibili request failed".to_string())));
    };
    let has_more = v
        .pointer("/data/has_more")
        .and_then(|x| x.as_i64())
        .unwrap_or(0)
        == 1;
    let list = v
        .pointer("/data/list")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for x in list {
        let rid = x
            .get("roomid")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let title = x
            .get("title")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        let cover = x
            .get("cover")
            .and_then(|y| y.as_str())
            .and_then(abs_cover)
            .map(|u| {
                if u.contains('@') {
                    u
                } else {
                    format!("{u}@400w.jpg")
                }
            });
        let user_name = x
            .get("uname")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let online = x.get("online").and_then(|y| y.as_i64());
        items.push(LiveRoomCard {
            site: Site::BiliLive,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover,
            user_name,
            online,
        });
    }

    Ok(LiveRoomList { has_more, items })
}

pub async fn search_rooms(
    client: &LiveDirectoryClient,
    keyword: &str,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    let kw = keyword.trim();
    if kw.is_empty() {
        return Err(LiveDirectoryError::InvalidInput(
            "keyword is empty".to_string(),
        ));
    }

    let base = client.cfg.endpoints.bili_api_base.trim_end_matches('/');
    let url = format!("{base}/x/web-interface/search/type");
    let v = get_json(
        client,
        &url,
        &[
            ("context".to_string(), "".to_string()),
            ("search_type".to_string(), "live".to_string()),
            ("cover_type".to_string(), "user_cover".to_string()),
            ("order".to_string(), "".to_string()),
            ("keyword".to_string(), kw.to_string()),
            ("category_id".to_string(), "".to_string()),
            ("__refresh__".to_string(), "".to_string()),
            ("_extra".to_string(), "".to_string()),
            ("highlight".to_string(), "0".to_string()),
            ("single_column".to_string(), "0".to_string()),
            ("page".to_string(), page.max(1).to_string()),
        ],
    )
    .await?;

    let list = v
        .pointer("/data/result/live_room")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for x in list {
        let rid = x
            .get("roomid")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let mut title = x
            .get("title")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        // remove <em> tags
        let re = regex::Regex::new(r"<.*?em.*?>").expect("em regex");
        title = re.replace_all(&title, "").to_string();
        let cover = x
            .get("cover")
            .and_then(|y| y.as_str())
            .and_then(abs_cover)
            .map(|u| {
                if u.contains('@') {
                    u
                } else {
                    format!("{u}@400w.jpg")
                }
            });
        let user_name = x
            .get("uname")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let online = x.get("online").and_then(|y| y.as_i64());
        items.push(LiveRoomCard {
            site: Site::BiliLive,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover,
            user_name,
            online,
        });
    }

    Ok(LiveRoomList {
        has_more: items.len() >= 40,
        items,
    })
}
