use tauri::State;

use crate::db::media::MediaItem;
use crate::error::JfgoatError;
use crate::state::AppState;

fn row_to_media_item(row: &rusqlite::Row) -> rusqlite::Result<MediaItem> {
    Ok(MediaItem {
        id: row.get(0)?,
        name: row.get(1)?,
        item_type: row.get(2)?,
        parent_id: row.get(3)?,
        series_id: row.get(4)?,
        series_name: row.get(5)?,
        season_id: row.get(6)?,
        season_name: row.get(7)?,
        index_number: row.get(8)?,
        production_year: row.get(9)?,
        overview: row.get(10)?,
        image_tag: row.get(11)?,
        backdrop_tag: row.get(12)?,
        date_created: row.get(13)?,
        date_updated: row.get(14)?,
        community_rating: row.get(15)?,
        official_rating: row.get(16)?,
        genres: row.get(17)?,
        run_time_ticks: row.get(18)?,
        played: row.get::<_, i32>(19)? != 0,
        play_count: row.get(20)?,
        playback_ticks: row.get(21)?,
        is_favorite: row.get::<_, i32>(22)? != 0,
        server_id: row.get(23)?,
    })
}

const SELECT_COLUMNS: &str = "id, name, type, parent_id, series_id, series_name,
     season_id, season_name, index_number, production_year,
     overview, image_tag, backdrop_tag, date_created, date_updated,
     community_rating, official_rating, genres, run_time_ticks,
     played, play_count, playback_ticks, is_favorite, server_id";

#[tauri::command]
pub fn get_recent_movies(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items WHERE type = 'Movie' ORDER BY date_created DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

#[tauri::command]
pub fn get_recent_series(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items WHERE type = 'Series' ORDER BY date_created DESC LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

#[tauri::command]
pub fn get_continue_watching(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE playback_ticks > 0 AND played = 0
         ORDER BY date_updated DESC
         LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

#[tauri::command]
pub fn get_latest_media(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    let db = state
        .db
        .lock()
        .map_err(|e| JfgoatError::Internal(e.to_string()))?;

    let sql = format!(
        "SELECT {} FROM media_items
         WHERE backdrop_tag IS NOT NULL AND type IN ('Movie', 'Series')
         ORDER BY date_created DESC
         LIMIT ?1",
        SELECT_COLUMNS
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![limit], |row| row_to_media_item(row))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}
