use serde_json::Value;

use crate::danmaku::model::Site;

use super::super::client::{LiveDirectoryClient, LiveDirectoryError};
use super::super::model::{LiveCategory, LiveRoomCard, LiveRoomList, LiveSubCategory};

fn make_input(room_id: &str) -> String {
    format!("douyu:{room_id}")
}

async fn get_json(
    client: &LiveDirectoryClient,
    url: &str,
    query: &[(String, String)],
) -> Result<Value, LiveDirectoryError> {
    let resp = client
        .http
        .get(url)
        .query(query)
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json::<Value>().await?)
}

fn parse_hot_num(s: &str) -> i64 {
    let raw = s.trim();
    if raw.is_empty() {
        return 0;
    }
    // Examples: "2.3万", "1234"
    if let Some(n) = raw.strip_suffix('万') {
        if let Ok(f) = n.trim().parse::<f64>() {
            return (f * 10_000.0) as i64;
        }
    }
    raw.parse::<i64>().unwrap_or(0)
}

fn random_hex_32(client: &LiveDirectoryClient) -> String {
    // Use EnvConfig rng so tests can be deterministic.
    let mut rng = client.cfg.env.rng.lock().expect("rng");
    let n = rng.u64(..);
    format!("{:x}", md5::compute(n.to_string()))
}

pub async fn get_categories(
    client: &LiveDirectoryClient,
) -> Result<Vec<LiveCategory>, LiveDirectoryError> {
    let base = client.cfg.endpoints.douyu_m_base.trim_end_matches('/');
    let url = format!("{base}/api/cate/list");
    let v = get_json(client, &url, &[]).await?;

    let cate1 = v
        .pointer("/data/cate1Info")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let cate2 = v
        .pointer("/data/cate2Info")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for c1 in cate1 {
        let id = c1
            .get("cate1Id")
            .and_then(|x| x.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        let name = c1
            .get("cate1Name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() || name.is_empty() {
            continue;
        }
        let mut subs = Vec::new();
        for c2 in &cate2 {
            let pid = c2
                .get("cate1Id")
                .and_then(|x| x.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            if pid != id {
                continue;
            }
            let sid = c2
                .get("cate2Id")
                .and_then(|x| x.as_i64())
                .map(|n| n.to_string())
                .unwrap_or_default();
            let sname = c2
                .get("cate2Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let pic = c2
                .get("icon")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            if sid.is_empty() || sname.is_empty() {
                continue;
            }
            subs.push(LiveSubCategory {
                id: sid,
                parent_id: id.clone(),
                name: sname,
                pic,
            });
        }
        out.push(LiveCategory {
            id,
            name,
            children: subs,
        });
    }

    out.sort_by_key(|c| c.id.parse::<i64>().unwrap_or(0));
    Ok(out)
}

pub async fn get_recommend_rooms(
    client: &LiveDirectoryClient,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    let base = client.cfg.endpoints.douyu_base.trim_end_matches('/');
    let url = format!("{base}/japi/weblist/apinc/allpage/6/{}", page.max(1));
    let v = get_json(client, &url, &[]).await?;

    let rl = v
        .pointer("/data/rl")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let mut items = Vec::new();
    for x in rl {
        if x.get("type").and_then(|t| t.as_i64()).unwrap_or(1) != 1 {
            continue;
        }
        let rid = x
            .get("rid")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let title = x
            .get("rn")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        let cover = x
            .get("rs16")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let user_name = x.get("nn").and_then(|y| y.as_str()).map(|s| s.to_string());
        let online = x.get("ol").and_then(|y| y.as_i64());
        items.push(LiveRoomCard {
            site: Site::Douyu,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover,
            user_name,
            online,
        });
    }

    let has_more = (v
        .pointer("/data/pgcnt")
        .and_then(|x| x.as_i64())
        .unwrap_or(0) as u32)
        > page.max(1);
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
    let base = client.cfg.endpoints.douyu_base.trim_end_matches('/');
    let url = format!("{base}/gapi/rkc/directory/mixList/2_{cid}/{}", page.max(1));
    let v = get_json(client, &url, &[]).await?;

    let rl = v
        .pointer("/data/rl")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for x in rl {
        if x.get("type").and_then(|t| t.as_i64()).unwrap_or(1) != 1 {
            continue;
        }
        let rid = x
            .get("rid")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let title = x
            .get("rn")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        let cover = x
            .get("rs16")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let user_name = x.get("nn").and_then(|y| y.as_str()).map(|s| s.to_string());
        let online = x.get("ol").and_then(|y| y.as_i64());
        items.push(LiveRoomCard {
            site: Site::Douyu,
            room_id: rid.clone(),
            input: make_input(&rid),
            title,
            cover,
            user_name,
            online,
        });
    }
    let pgcnt = v
        .pointer("/data/pgcnt")
        .and_then(|x| x.as_i64())
        .unwrap_or(0);
    let has_more = (page.max(1) as i64) < pgcnt;
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
    let base = client.cfg.endpoints.douyu_base.trim_end_matches('/');
    let url = format!("{base}/japi/search/api/searchShow");
    let did = random_hex_32(client);
    let resp = client
        .http
        .get(&url)
        .query(&[
            ("kw".to_string(), kw.to_string()),
            ("page".to_string(), page.max(1).to_string()),
            ("pageSize".to_string(), "20".to_string()),
        ])
        .header(reqwest::header::REFERER, "https://www.douyu.com/search/")
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36",
        )
        .header(reqwest::header::COOKIE, format!("dy_did={did};acf_did={did}"))
        .send()
        .await?
        .error_for_status()?;
    let v = resp.json::<Value>().await?;

    let items_raw = v
        .pointer("/data/relateShow")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut items = Vec::new();
    for x in items_raw {
        let rid = x
            .get("rid")
            .and_then(|y| y.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        if rid.is_empty() {
            continue;
        }
        let title = x
            .get("roomName")
            .and_then(|y| y.as_str())
            .unwrap_or("")
            .to_string();
        let cover = x
            .get("roomSrc")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let user_name = x
            .get("nickName")
            .and_then(|y| y.as_str())
            .map(|s| s.to_string());
        let hot = x.get("hot").and_then(|y| y.as_str()).unwrap_or("");
        let online = Some(parse_hot_num(hot));
        items.push(LiveRoomCard {
            site: Site::Douyu,
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
