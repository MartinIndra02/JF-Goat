use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::api::media::PaginatedResult as ApiPaginatedResult;
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
    pub user_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct PaginationScope {
    pub start_index: u32,
    pub limit: u32,
}

pub fn to_paginated_result(
    items: Vec<MediaItem>,
    pagination: PaginationScope,
    total_record_count: Option<u32>,
) -> ApiPaginatedResult<MediaItem> {
    let inferred_total = pagination
        .start_index
        .saturating_add(items.len() as u32);
    let total = total_record_count.unwrap_or(inferred_total);
    let has_more = match total_record_count {
        Some(known_total) => inferred_total < known_total,
        None => pagination.limit > 0 && (items.len() as u32) >= pagination.limit,
    };

    ApiPaginatedResult {
        items,
        total_record_count: total,
        start_index: pagination.start_index,
        limit: pagination.limit,
        has_more,
    }
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
                played, play_count, playback_ticks, is_favorite, server_id, user_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19,
                ?20, ?21, ?22, ?23, ?24, ?25
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
                item.user_id,
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
    server_id: &str,
    user_id: &str,
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
                m.played, m.play_count, m.playback_ticks, m.is_favorite, m.server_id, m.user_id
         FROM media_items m
         JOIN media_items_fts fts ON m.rowid = fts.rowid
         WHERE media_items_fts MATCH ?1
           AND m.server_id = ?2
           AND m.user_id = ?3
         ORDER BY rank
         LIMIT ?4",
    )?;

    let rows = stmt.query_map(rusqlite::params![fts_query, server_id, user_id, limit], |row| {
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
            user_id: row.get(24)?,
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
    get_local_item_count_scoped(conn, None, None)
}

/// Get the total count of media items in the local database for a specific scope.
pub fn get_local_item_count_scoped(
    conn: &Connection,
    server_id: Option<&str>,
    user_id: Option<&str>,
) -> Result<u32, JfgoatError> {
    if let (Some(server_id), Some(user_id)) = (server_id, user_id) {
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM media_items WHERE server_id = ?1 AND user_id = ?2",
            rusqlite::params![server_id, user_id],
            |row| row.get(0),
        )?;
        return Ok(count);
    }

    let count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM media_items",
        [],
        |row| row.get(0),
    )?;
    Ok(count)
}

/// Checkpoint status for a library view.
pub enum CheckpointStatus {
    /// View has been fully synced.
    Completed,
    /// Sync is in progress; `last_index` is the resume offset.
    InProgress(u32),
    /// No checkpoint exists for this view.
    NotFound,
}

/// Read the sync checkpoint for a specific view.
pub fn get_checkpoint(
    conn: &Connection,
    view_id: &str,
    server_id: &str,
    user_id: &str,
) -> Result<CheckpointStatus, JfgoatError> {
    let result = conn.query_row(
        "SELECT status, last_index FROM sync_checkpoints WHERE view_id = ?1 AND server_id = ?2 AND user_id = ?3",
        rusqlite::params![view_id, server_id, user_id],
        |row| {
            let status: String = row.get(0)?;
            let last_index: u32 = row.get(1)?;
            Ok((status, last_index))
        },
    );

    match result {
        Ok((status, last_index)) => {
            if status == "COMPLETED" {
                Ok(CheckpointStatus::Completed)
            } else {
                Ok(CheckpointStatus::InProgress(last_index))
            }
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(CheckpointStatus::NotFound),
        Err(e) => Err(e.into()),
    }
}

/// Create or reset a checkpoint for a view to IN_PROGRESS at index 0.
pub fn init_checkpoint(
    conn: &Connection,
    view_id: &str,
    server_id: &str,
    user_id: &str,
) -> Result<(), JfgoatError> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_checkpoints (view_id, status, last_index, server_id, user_id) VALUES (?1, 'IN_PROGRESS', 0, ?2, ?3)",
        rusqlite::params![view_id, server_id, user_id],
    )?;
    Ok(())
}

/// Advance the checkpoint's last_index for a view.
pub fn update_checkpoint_index(
    conn: &Connection,
    view_id: &str,
    server_id: &str,
    user_id: &str,
    last_index: u32,
) -> Result<(), JfgoatError> {
    conn.execute(
        "UPDATE sync_checkpoints SET last_index = ?1 WHERE view_id = ?2 AND server_id = ?3 AND user_id = ?4",
        rusqlite::params![last_index, view_id, server_id, user_id],
    )?;
    Ok(())
}

/// Mark a view's checkpoint as COMPLETED.
pub fn complete_checkpoint(
    conn: &Connection,
    view_id: &str,
    server_id: &str,
    user_id: &str,
) -> Result<(), JfgoatError> {
    conn.execute(
        "UPDATE sync_checkpoints SET status = 'COMPLETED' WHERE view_id = ?1 AND server_id = ?2 AND user_id = ?3",
        rusqlite::params![view_id, server_id, user_id],
    )?;
    Ok(())
}

/// Clear all checkpoints (used when starting a fresh sync).
pub fn clear_all_checkpoints(
    conn: &Connection,
    server_id: &str,
    user_id: &str,
) -> Result<(), JfgoatError> {
    conn.execute(
        "DELETE FROM sync_checkpoints WHERE server_id = ?1 AND user_id = ?2",
        rusqlite::params![server_id, user_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        clear_all_checkpoints, complete_checkpoint, get_checkpoint, init_checkpoint,
        insert_media_chunk, search_local, update_checkpoint_index, CheckpointStatus,
        MediaItem,
    };
    use crate::db::init_db;
    use rusqlite::Connection;

    fn sample_item(id: &str, name: &str, item_type: &str) -> MediaItem {
        MediaItem {
            id: id.to_string(),
            name: name.to_string(),
            item_type: item_type.to_string(),
            parent_id: None,
            series_id: Some("series-1".to_string()),
            series_name: Some("My Show".to_string()),
            season_id: Some("season-1".to_string()),
            season_name: Some("Season 1".to_string()),
            index_number: Some(1),
            production_year: Some(2024),
            overview: Some("Test overview".to_string()),
            image_tag: None,
            backdrop_tag: None,
            date_created: Some("2024-01-01T00:00:00.000Z".to_string()),
            date_updated: Some("2024-01-01T00:00:00.000Z".to_string()),
            community_rating: Some(8.0),
            official_rating: Some("TV-14".to_string()),
            genres: Some("Drama".to_string()),
            run_time_ticks: Some(12_000_000_000),
            played: false,
            play_count: 0,
            playback_ticks: 0,
            is_favorite: false,
            server_id: "srv-1".to_string(),
            user_id: "user-1".to_string(),
        }
    }

    #[test]
    fn search_local_returns_prefix_matches_and_empty_for_blank_query() {
        let conn = Connection::open_in_memory().expect("in-memory database should open");
        init_db(&conn).expect("database schema should initialize");

        let items = vec![
            sample_item("ep-1", "Pilot", "Episode"),
            sample_item("ep-2", "Nexus", "Episode"),
            sample_item("movie-1", "Signal Fire", "Movie"),
        ];
        insert_media_chunk(&conn, &items).expect("seed media items should insert");

        let result = search_local(&conn, "Pil", "srv-1", "user-1", 10)
            .expect("search should succeed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "ep-1");

        let empty = search_local(&conn, "   ", "srv-1", "user-1", 10)
            .expect("blank query should succeed");
        assert!(empty.is_empty());
    }

    #[test]
    fn checkpoint_lifecycle_transitions_correctly() {
        let conn = Connection::open_in_memory().expect("in-memory database should open");
        init_db(&conn).expect("database schema should initialize");

        let server_id = "srv-1";
        let user_id = "user-1";

        match get_checkpoint(&conn, "lib-1", server_id, user_id)
            .expect("checkpoint lookup should succeed")
        {
            CheckpointStatus::NotFound => {}
            _ => panic!("expected checkpoint to be missing before initialization"),
        }

        init_checkpoint(&conn, "lib-1", server_id, user_id)
            .expect("checkpoint should initialize");
        match get_checkpoint(&conn, "lib-1", server_id, user_id)
            .expect("checkpoint lookup should succeed")
        {
            CheckpointStatus::InProgress(last_index) => assert_eq!(last_index, 0),
            _ => panic!("expected in-progress checkpoint after initialization"),
        }

        update_checkpoint_index(&conn, "lib-1", server_id, user_id, 42)
            .expect("checkpoint index update should succeed");
        match get_checkpoint(&conn, "lib-1", server_id, user_id)
            .expect("checkpoint lookup should succeed")
        {
            CheckpointStatus::InProgress(last_index) => assert_eq!(last_index, 42),
            _ => panic!("expected in-progress checkpoint after index update"),
        }

        complete_checkpoint(&conn, "lib-1", server_id, user_id)
            .expect("checkpoint completion should succeed");
        match get_checkpoint(&conn, "lib-1", server_id, user_id)
            .expect("checkpoint lookup should succeed")
        {
            CheckpointStatus::Completed => {}
            _ => panic!("expected completed checkpoint after completion"),
        }

        clear_all_checkpoints(&conn, server_id, user_id)
            .expect("checkpoint clear should succeed");
        match get_checkpoint(&conn, "lib-1", server_id, user_id)
            .expect("checkpoint lookup should succeed")
        {
            CheckpointStatus::NotFound => {}
            _ => panic!("expected checkpoint to be removed after clearing"),
        }
    }
}
