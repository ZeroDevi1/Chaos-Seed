#[derive(Debug, Clone)]
pub struct DouyuEncryption {
    pub key: String,
    pub rand_str: String,
    pub enc_time: i32,
    pub enc_data: String,
    pub is_special: i32,
}

fn md5_hex(s: &str) -> String {
    format!("{:x}", md5::compute(s))
}

impl DouyuEncryption {
    /// Ported from IINA+ `DouyuEncryption.auth`.
    pub fn auth(&self, rid: &str, ts: i64) -> String {
        let mut u = self.rand_str.clone();
        for _ in 0..self.enc_time.max(0) {
            u = md5_hex(&(u + &self.key));
        }
        let o = if self.is_special == 1 {
            String::new()
        } else {
            format!("{rid}{ts}")
        };
        md5_hex(&(u + &self.key + &o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_matches_expected() {
        let enc = DouyuEncryption {
            key: "k".to_string(),
            rand_str: "r".to_string(),
            enc_time: 2,
            enc_data: "e".to_string(),
            is_special: 0,
        };
        // Manual expansion:
        // u0="r"
        // u1=md5("rk")
        // u2=md5(u1+"k")
        // auth=md5(u2+"k"+"ridts")
        let ts = 123_i64;
        let u1 = format!("{:x}", md5::compute("rk"));
        let u2 = format!("{:x}", md5::compute(format!("{u1}k")));
        let expected = format!("{:x}", md5::compute(format!("{u2}kroom{ts}")));
        assert_eq!(enc.auth("room", ts), expected);
    }

    #[test]
    fn auth_special_omits_rid_ts() {
        let enc = DouyuEncryption {
            key: "k".to_string(),
            rand_str: "r".to_string(),
            enc_time: 0,
            enc_data: "e".to_string(),
            is_special: 1,
        };
        let expected = format!("{:x}", md5::compute("rk"));
        assert_eq!(enc.auth("room", 1), expected);
    }
}
