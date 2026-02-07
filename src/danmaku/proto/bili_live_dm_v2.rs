//! Minimal subset of BiliLive dm_v2 protobuf used by IINA+.
//!
//! Source reference: `refs/iina-plus/IINA+/Utils/Danmaku/BiliLiveDMv2.pb.swift`

use prost::Message;

#[derive(Clone, PartialEq, Message)]
pub struct Dm {
    /// tag=6
    #[prost(string, tag = "6")]
    pub text: String,

    /// tag=11
    #[prost(enumeration = "BizScene", tag = "11")]
    pub biz_scene: i32,

    /// tag=13
    #[prost(enumeration = "DmType", tag = "13")]
    pub dm_type: i32,

    /// tag=14: repeated map-entry (swift generator uses repeated messages)
    #[prost(message, repeated, tag = "14")]
    pub emoticons: Vec<EmotsTemp>,
}

#[derive(Clone, PartialEq, Message)]
pub struct EmotsTemp {
    #[prost(string, tag = "1")]
    pub key: String,

    #[prost(message, optional, tag = "2")]
    pub value: Option<Emoticon>,
}

#[derive(Clone, PartialEq, Message)]
pub struct Emoticon {
    #[prost(string, tag = "1")]
    pub unique: String,
    #[prost(string, tag = "2")]
    pub url: String,
    #[prost(bool, tag = "3")]
    pub is_dynamic: bool,
    #[prost(int64, tag = "4")]
    pub in_player_area: i64,
    #[prost(int64, tag = "5")]
    pub bulge_display: i64,
    #[prost(int64, tag = "6")]
    pub height: i64,
    #[prost(int64, tag = "7")]
    pub width: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum BizScene {
    None = 0,
    Lottery = 1,
    Survive = 2,
    VoiceConn = 3,
    PlayBack = 4,
    Vote = 5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub enum DmType {
    Normal = 0,
    Emoticon = 1,
    Voice = 2,
}

