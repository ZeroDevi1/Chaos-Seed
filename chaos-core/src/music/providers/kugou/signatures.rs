use std::collections::BTreeMap;

pub fn md5_hex_lower(s: &str) -> String {
    let digest = md5::compute(s.as_bytes());
    format!("{:x}", digest)
}

pub fn signature_web_params(params: &BTreeMap<String, String>) -> String {
    let salt = "NVPh5oo715z5DIWAeQlhMDsWXXQV4hwt";
    let params_string = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("");
    md5_hex_lower(&format!("{salt}{params_string}{salt}"))
}

pub fn signature_android_params(params: &BTreeMap<String, String>, data: Option<&str>) -> String {
    let salt = "OIlwieks28dk2k092lksi2UIkp";
    let params_string = params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("");
    md5_hex_lower(&format!("{salt}{params_string}{}{salt}", data.unwrap_or("")))
}

pub fn signature_register_params(params: &BTreeMap<String, String>) -> String {
    let mut vals = params.values().cloned().collect::<Vec<_>>();
    vals.sort();
    let params_string = vals.join("");
    md5_hex_lower(&format!("1014{params_string}1014"))
}

pub fn sign_key(hash: &str, mid: &str, userid: &str, appid: &str) -> String {
    let salt = "57ae12eb6890223e355ccfcb74edf70d";
    md5_hex_lower(&format!("{hash}{salt}{appid}{mid}{userid}"))
}

pub fn sign_key_lite(hash: &str, mid: &str, userid: &str, appid: &str) -> String {
    let salt = "185672dd44712f60bb1736df5a377e82";
    md5_hex_lower(&format!("{hash}{salt}{appid}{mid}{userid}"))
}
