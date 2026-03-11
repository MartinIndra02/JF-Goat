use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::JfgoatError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaItem {
    pub id: String,
    pub name: String,
    #[serde(alias = "Type", rename = "type")]
    pub item_type: String,
    pub parent_id: Option<String>,
    pub series_id: Option<String>,
    pub series_name: Option<String>,
    pub season_id: Option<String>,
    pub season_name: Option<String>,
    pub index_number: Option<i64>,
    pub production_year: Option<i64>,
    pub overview: Option<String>,
    pub image_tag: Option<String>,
    pub backdrop_tag: Option<String>,
    pub date_created: Option<String>,
    pub date_updated: Option<String>,
    pub community_rating: Option<f64>,
    pub official_rating: Option<String>,
    pub genres: Option<String>,
    pub run_time_ticks: Option<i64>,
    pub played: bool,
    pub play_count: i64,
    pub playback_ticks: i64,
    pub is_favorite: bool,
    pub server_id: String,
}

/// Insert a chunk of media items in a single transaction for maximum I/O performance.
pub fn insert_media_chunk(
    conn: &Connection,
    items: &[MediaItem],
) -> Result<(), JfgoatError> {
    let tx = conn.unchecked_transaction()?;

    {
        let mut stmt = tx.prepare_cached(
            "INSERT OR REPLACE INTO media_items (
                id, name, type, parent_id, series_id, series_name,
                season_id, season_name, index_number, production_year,
                overview, image_tag, backdrop_tag, date_created, date_updated,
                community_rating, official_rating, genres, run_time_ticks,
                played, play_count, playback_ticks, is_favorite, server_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19,
                ?20, ?21, ?22, ?23, ?24
            )",
        )?;

        for item in items {
            stmt.execute(rusqlite::params![
                item.id,
                item.name,
                item.item_type,
                item.parent_id,
                item.series_id,
                item.series_name,
                item.season_id,
                item.season_name,
                item.index_number,
                item.production_year,
                item.overview,
                item.image_tag,
                item.backdrop_tag,
                item.date_created,
                item.date_updated,
                item.community_rating,
                item.official_rating,
                item.genres,
                item.run_time_ticks,
                item.played as i32,
                item.play_count,
                item.playback_ticks,
                item.is_favorite as i32,
                item.server_id,
            ])?;
        }
    }

    tx.commit()?;
    Ok(())
}

/// Search media items using FTS5 full-text search (sub-millisecond on 100k+ items).
pub fn search_local(
    conn: &Connection,
    query: &str,
    limit: u32,
) -> Result<Vec<MediaItem>, JfgoatError> {
    // Escape FTS5 special characters and append wildcard for prefix matching
    let sanitized = query
        .replace('"', "\"\"")
        .trim()
        .to_string();

    if sanitized.is_empty() {
        return Ok(vec![]);
    }

    let fts_query = format!("\"{}\"*", sanitized);

    let mut stmt = conn.prepare(
        "SELECT m.id, m.name, m.type, m.parent_id, m.series_id, m.series_name,
                m.season_id, m.season_name, m.index_number, m.production_year,
                m.overview, m.image_tag, m.backdrop_tag, m.date_created, m.date_updated,
                m.community_rating, m.official_rating, m.genres, m.run_time_ticks,
                m.played, m.play_count, m.playback_ticks, m.is_favorite, m.server_id
         FROM media_items m
         JOIN media_items_fts fts ON m.rowid = fts.rowid
         WHERE media_items_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(rusqlite::params![fts_query, limit], |row| {
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
    })?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

/// Get the total count of media items in the local database.
pub fn get_local_item_count(conn: &Connection) -> Result<u32, JfgoatError> {
    let count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM media_items",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}
