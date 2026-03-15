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

/// Fetch the absolute total item count across the user's entire library.
/// Uses Limit=1 so the response body is minimal; we only care about TotalRecordCount.
pub async fn fetch_total_item_count(
    client: &JellyfinClient,
    user_id: &str,
) -> Result<u32, JfgoatError> {
    let path = format!(
        "/Users/{}/Items?Limit=1&Recursive=true&EnableTotalRecordCount=true",
        user_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch total item count: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse total item count response: {}", e))
    })?;

    Ok(data.total_record_count)
}

/// Fetch items the user is currently watching (have playback progress).
/// Uses the Jellyfin `/Users/{id}/Items/Resume` endpoint.
pub async fn fetch_resume_items(
    client: &JellyfinClient,
    user_id: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/Resume?Limit={}&Recursive=true\
         &Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &IncludeItemTypes=Movie,Episode\
         &EnableTotalRecordCount=false\
         &MediaTypes=Video",
        user_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch resume items: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse resume items response: {}", e))
    })?;

    Ok(data)
}

/// Fetch the next episodes to watch across all series.
/// Uses the Jellyfin `/Shows/NextUp` endpoint.
pub async fn fetch_next_up(
    client: &JellyfinClient,
    user_id: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Shows/NextUp?UserId={}&Limit={}\
         &Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &EnableTotalRecordCount=false",
        user_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch next up: status {}",
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse next up response: {}", e))
    })?;

    Ok(data)
}

/// Fetch the latest items added to a specific library view.
/// Uses the Jellyfin `/Users/{id}/Items/Latest` endpoint.
pub async fn fetch_latest_items(
    client: &JellyfinClient,
    user_id: &str,
    parent_id: &str,
    limit: u32,
) -> Result<Vec<JellyfinItem>, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/Latest?ParentId={}&Limit={}\
         &Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags\
         &GroupItems=true",
        user_id, parent_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch latest items: status {}",
            resp.status()
        )));
    }

    // The /Latest endpoint returns a flat array of items, not wrapped in { Items: [...] }
    let data: Vec<JellyfinItem> = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse latest items response: {}", e))
    })?;

    Ok(data)
}

/// Fetch a single item by ID from the Jellyfin server.
/// Used as fallback when item isn't in local DB (pre-sync).
pub async fn fetch_item_by_id(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<JellyfinItem, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/{}?Fields=Overview,Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags",
        user_id, item_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch item {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let item: JellyfinItem = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse item response: {}", e))
    })?;

    Ok(item)
}

/// Fetch seasons for a series from the Jellyfin server.
/// Used as fallback when seasons aren't in local DB (pre-sync).
pub async fn fetch_seasons(
    client: &JellyfinClient,
    user_id: &str,
    series_id: &str,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Shows/{}/Seasons?UserId={}&Fields=Overview,Genres,DateCreated,ProductionYear,ImageTags,BackdropImageTags",
        series_id, user_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch seasons for {}: status {}",
            series_id,
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse seasons response: {}", e))
    })?;

    Ok(data)
}

/// Fetch episodes for a season from the Jellyfin server.
/// Used as fallback when episodes aren't in local DB (pre-sync).
pub async fn fetch_episodes(
    client: &JellyfinClient,
    user_id: &str,
    series_id: &str,
    season_id: &str,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Shows/{}/Episodes?SeasonId={}&UserId={}&Fields=Overview,Genres,DateCreated,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags",
        series_id, season_id, user_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch episodes for season {}: status {}",
            season_id,
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse episodes response: {}", e))
    })?;

    Ok(data)
}

/// Jellyfin person object returned from the People field or /Items/{id} endpoint.
#[derive(Debug, Deserialize)]
pub struct JellyfinPerson {
    #[serde(alias = "Id")]
    pub id: String,
    #[serde(alias = "Name", default)]
    pub name: Option<String>,
    #[serde(alias = "Role")]
    pub role: Option<String>,
    #[serde(alias = "Type")]
    pub person_type: Option<String>,
    #[serde(alias = "PrimaryImageTag")]
    pub primary_image_tag: Option<String>,
}

/// Jellyfin item response that includes People field.
#[derive(Debug, Deserialize)]
pub struct JellyfinItemWithPeople {
    #[serde(alias = "People", default)]
    pub people: Vec<JellyfinPerson>,
}

/// Fetch the cast & crew (people) for a specific item from the Jellyfin server.
pub async fn fetch_item_people(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
) -> Result<Vec<JellyfinPerson>, JfgoatError> {
    let path = format!(
        "/Users/{}/Items/{}?Fields=People",
        user_id, item_id
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch people for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: JellyfinItemWithPeople = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse people response: {}", e))
    })?;

    Ok(data.people)
}

/// Fetch similar/related items for a specific item from the Jellyfin server.
pub async fn fetch_similar_items(
    client: &JellyfinClient,
    user_id: &str,
    item_id: &str,
    limit: u32,
) -> Result<JellyfinItemsResponse, JfgoatError> {
    let path = format!(
        "/Items/{}/Similar?UserId={}&Limit={}&Fields=Overview,Genres,ProductionYear,CommunityRating,OfficialRating,RunTimeTicks,ImageTags,BackdropImageTags",
        item_id, user_id, limit
    );

    let resp = client.get(&path).await?;

    if !resp.status().is_success() {
        return Err(JfgoatError::Http(format!(
            "Failed to fetch similar items for {}: status {}",
            item_id,
            resp.status()
        )));
    }

    let data: JellyfinItemsResponse = resp.json().await.map_err(|e| {
        JfgoatError::Http(format!("Failed to parse similar items response: {}", e))
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
