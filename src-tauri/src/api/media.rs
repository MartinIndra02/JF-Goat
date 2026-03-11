use serde::Deserialize;

use crate::api::client::JellyfinClient;
use crate::error::JfgoatError;

#[derive(Debug, Deserialize)]
pub struct JellyfinItemsResponse {
    #[serde(alias = "Items", default)]
    pub items: Vec<JellyfinItem>,
    #[serde(alias = "TotalRecordCount", default)]
    pub total_record_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinViewsResponse {
    #[serde(alias = "Items", default)]
    pub items: Vec<JellyfinView>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinView {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name", default)]
    pub name: Option<String>,
    #[serde(alias = "CollectionType", default)]
    pub collection_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinItem {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name", default)]
    pub name: Option<String>,
    #[serde(alias = "Type")]
    pub item_type: String,
    #[serde(alias = "ParentId")]
    pub parent_id: Option<String>,
    #[serde(alias = "SeriesId")]
    pub series_id: Option<String>,
    #[serde(alias = "SeriesName")]
    pub series_name: Option<String>,
    #[serde(alias = "SeasonId")]
    pub season_id: Option<String>,
    #[serde(alias = "SeasonName")]
    pub season_name: Option<String>,
    #[serde(alias = "IndexNumber")]
    pub index_number: Option<i64>,
    #[serde(alias = "ProductionYear")]
    pub production_year: Option<i64>,
    #[serde(alias = "Overview")]
    pub overview: Option<String>,
    #[serde(alias = "ImageTags")]
    pub image_tags: Option<ImageTags>,
    #[serde(alias = "BackdropImageTags")]
    pub backdrop_image_tags: Option<Vec<String>>,
    #[serde(alias = "DateCreated")]
    pub date_created: Option<String>,
    #[serde(alias = "DateLastMediaAdded", alias = "PremiereDate")]
    pub date_updated: Option<String>,
    #[serde(alias = "CommunityRating")]
    pub community_rating: Option<f64>,
    #[serde(alias = "OfficialRating")]
    pub official_rating: Option<String>,
    #[serde(alias = "Genres")]
    pub genres: Option<Vec<String>>,
    #[serde(alias = "RunTimeTicks")]
    pub run_time_ticks: Option<i64>,
    #[serde(alias = "UserData")]
    pub user_data: Option<JellyfinUserData>,
}

#[derive(Debug, Deserialize)]
pub struct ImageTags {
    #[serde(alias = "Primary")]
    pub primary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JellyfinUserData {
    #[serde(alias = "Played")]
    pub played: Option<bool>,
    #[serde(alias = "PlayCount")]
    pub play_count: Option<i64>,
    #[serde(alias = "PlaybackPositionTicks")]
    pub playback_position_ticks: Option<i64>,
    #[serde(alias = "IsFavorite")]
    pub is_favorite: Option<bool>,
}

/// Fetch the user's top-level libraries (Views).
pub async fn fetch_user_views(
    client: &JellyfinClient,
    user_id: &str,
) -> Result<JellyfinViewsResponse, JfgoatError> {
    let path = format!("/Users/{}/Views", user_id);
    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch views: status {}",
            resp.status()
        )));
    }

    let data: JellyfinViewsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse views response: {}", e))
    })?;

    Ok(data)
}

/// Fetch items scoped to a specific library view (with total count).
/// Use for the first request per view to get the total record count.
pub async fn fetch_view_items(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    fetch_view_items_inner(client, user_id, view_id, start_index, limit, true).await
}

/// Fetch items scoped to a specific library view (without total count).
/// Use for all subsequent pagination requests after the first per view.
pub async fn fetch_view_items_no_count(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    fetch_view_items_inner(client, user_id, view_id, start_index, limit, false).await
}

async fn fetch_view_items_inner(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
    enable_total_count: bool,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?ParentId={}&StartIndex={}&Limit={}&Recursive=true\
         &IncludeItemTypes=Movie,Series,Episode,Season,BoxSet,MusicAlbum,MusicArtist,Audio\
         &Fields=Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount={}",
        user_id, view_id, start_index, limit, enable_total_count
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch items: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse items response: {}", e))
    })?;

    Ok(data)
}

/// Fetch only Series items for a TV Shows library view (with total count).
pub async fn fetch_series(
    client: &JellyfinClient,
    user_id: &str,
    view_id: &str,
    start_index: u32,
    limit: u32,
    enable_total_count: bool,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?ParentId={}&StartIndex={}&Limit={}&Recursive=false\
         &IncludeItemTypes=Series\
         &Fields=Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount={}",
        user_id, view_id, start_index, limit, enable_total_count
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch series: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse series response: {}", e))
    })?;

    Ok(data)
}

/// Fetch Seasons and Episodes for a specific Series.
pub async fn fetch_series_children(
    client: &JellyfinClient,
    user_id: &str,
    series_id: &str,
    start_index: u32,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?ParentId={}&StartIndex={}&Limit={}&Recursive=true\
         &IncludeItemTypes=Season,Episode\
         &Fields=Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount=false",
        user_id, series_id, start_index, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch series children: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse series children response: {}", e))
    })?;

    Ok(data)
}

/// Search items directly on the remote Jellyfin server (fallback during INITIAL_SYNC).
pub async fn search_remote(
    client: &JellyfinClient,
    user_id: &str,
    query: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?searchTerm={}&Limit={}&Recursive=true&Fields=Overview,Genres,ProductionYear,ImageTags",
        user_id,
        urlencoding::encode(query),
        limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Remote search failed: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse search response: {}", e))
    })?;

    Ok(data)
}
