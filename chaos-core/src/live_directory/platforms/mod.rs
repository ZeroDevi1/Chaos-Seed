use crate::danmaku::model::Site;

use super::client::{LiveDirectoryClient, LiveDirectoryError};
use super::model::{LiveCategory, LiveRoomList};

mod bili_live;
mod douyu;
mod huya;

pub async fn get_categories(
    client: &LiveDirectoryClient,
    site: Site,
) -> Result<Vec<LiveCategory>, LiveDirectoryError> {
    match site {
        Site::BiliLive => bili_live::get_categories(client).await,
        Site::Douyu => douyu::get_categories(client).await,
        Site::Huya => huya::get_categories(client).await,
    }
}

pub async fn get_recommend_rooms(
    client: &LiveDirectoryClient,
    site: Site,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    match site {
        Site::BiliLive => bili_live::get_recommend_rooms(client, page).await,
        Site::Douyu => douyu::get_recommend_rooms(client, page).await,
        Site::Huya => huya::get_recommend_rooms(client, page).await,
    }
}

pub async fn get_category_rooms(
    client: &LiveDirectoryClient,
    site: Site,
    parent_id: Option<&str>,
    category_id: &str,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    match site {
        Site::BiliLive => bili_live::get_category_rooms(client, parent_id, category_id, page).await,
        Site::Douyu => douyu::get_category_rooms(client, category_id, page).await,
        Site::Huya => huya::get_category_rooms(client, category_id, page).await,
    }
}

pub async fn search_rooms(
    client: &LiveDirectoryClient,
    site: Site,
    keyword: &str,
    page: u32,
) -> Result<LiveRoomList, LiveDirectoryError> {
    match site {
        Site::BiliLive => bili_live::search_rooms(client, keyword, page).await,
        Site::Douyu => douyu::search_rooms(client, keyword, page).await,
        Site::Huya => huya::search_rooms(client, keyword, page).await,
    }
}
