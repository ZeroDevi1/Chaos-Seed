use serde::{Deserialize, Serialize};

use crate::danmaku::model::Site;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveSubCategory {
    pub id: String,
    pub parent_id: String,
    pub name: String,
    pub pic: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveCategory {
    pub id: String,
    pub name: String,
    pub children: Vec<LiveSubCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRoomCard {
    pub site: Site,
    pub room_id: String,
    /// `bilibili:<rid>` / `huya:<rid>` / `douyu:<rid>`
    pub input: String,
    pub title: String,
    pub cover: Option<String>,
    pub user_name: Option<String>,
    pub online: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveRoomList {
    pub has_more: bool,
    pub items: Vec<LiveRoomCard>,
}
