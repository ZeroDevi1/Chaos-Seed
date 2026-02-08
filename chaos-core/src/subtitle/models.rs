use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ThunderSubtitleItem {
    #[serde(default)]
    pub gcid: String,
    #[serde(default)]
    pub cid: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub ext: String,
    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub source: i64,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub fingerprintf_score: f64,
    #[serde(default)]
    pub extra_name: String,
    #[serde(default)]
    pub mt: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThunderSubtitleResponse {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub data: Vec<ThunderSubtitleItem>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_gate_fields() {
        let raw = r#"
        {
          "code": 0,
          "result": "ok",
          "data": [{
            "gcid": "g",
            "cid": "c",
            "url": "https://example.com/a.srt",
            "ext": "srt",
            "name": "Example",
            "duration": 1,
            "languages": ["zh", "en"],
            "source": 0,
            "score": 9.5,
            "fingerprintf_score": 0.1,
            "extra_name": "extra",
            "mt": 0
          }]
        }
        "#;
        let resp: ThunderSubtitleResponse = serde_json::from_str(raw).expect("json");
        assert_eq!(resp.code, 0);
        assert_eq!(resp.result, "ok");
        assert_eq!(resp.data.len(), 1);
        assert_eq!(resp.data[0].name, "Example");
        assert_eq!(
            resp.data[0].languages,
            vec!["zh".to_string(), "en".to_string()]
        );
    }
}
