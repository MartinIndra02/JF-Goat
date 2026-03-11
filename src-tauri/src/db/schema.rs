use rusqlite::Connection;

use crate::error::JfgoatError;

pub fn init_db(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        INSERT OR IGNORE INTO metadata (key, value) VALUES ('schema_version', '1');",
    )?;

    migrate(conn)?;
    Ok(())
}

fn migrate(conn: &Connection) -> Result<(), JfgoatError> {
    let version: String = conn.query_row(
        "SELECT value FROM metadata WHERE key = 'schema_version'",
        [],
        |row| row.get(0),
    )?;

    if version == "1" {
        migrate_v1_to_v2(conn)?;
        migrate_v2_to_v3(conn)?;
        migrate_v3_to_v4(conn)?;
    } else if version == "2" {
        migrate_v2_to_v3(conn)?;
        migrate_v3_to_v4(conn)?;
    } else if version == "3" {
        migrate_v3_to_v4(conn)?;
    }

    Ok(())
}

fn migrate_v1_to_v2(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS servers (
            id           TEXT PRIMARY KEY,
            name         TEXT NOT NULL,
            url          TEXT NOT NULL,
            user_id      TEXT,
            username     TEXT,
            is_active    INTEGER NOT NULL DEFAULT 1,
            connected_at TEXT NOT NULL
        );

        INSERT OR IGNORE INTO metadata (key, value)
            SELECT 'device_id', lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab', abs(random()) % 4 + 1, 1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6)))
            WHERE NOT EXISTS (SELECT 1 FROM metadata WHERE key = 'device_id');

        UPDATE metadata SET value = '2' WHERE key = 'schema_version';",
    )?;

    println!("Database migrated to v2");
    Ok(())
}

fn migrate_v2_to_v3(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS media_items (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            type            TEXT NOT NULL,
            parent_id       TEXT,
            series_id       TEXT,
            series_name     TEXT,
            season_id       TEXT,
            season_name     TEXT,
            index_number    INTEGER,
            production_year INTEGER,
            overview        TEXT,
            image_tag       TEXT,
            backdrop_tag    TEXT,
            date_created    TEXT,
            date_updated    TEXT,
            community_rating REAL,
            official_rating TEXT,
            genres          TEXT,
            run_time_ticks  INTEGER,
            played          INTEGER NOT NULL DEFAULT 0,
            play_count      INTEGER NOT NULL DEFAULT 0,
            playback_ticks  INTEGER NOT NULL DEFAULT 0,
            is_favorite     INTEGER NOT NULL DEFAULT 0,
            server_id       TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_media_type ON media_items(type);
        CREATE INDEX IF NOT EXISTS idx_media_parent ON media_items(parent_id);
        CREATE INDEX IF NOT EXISTS idx_media_series ON media_items(series_id);
        CREATE INDEX IF NOT EXISTS idx_media_date_updated ON media_items(date_updated);

        CREATE VIRTUAL TABLE IF NOT EXISTS media_items_fts USING fts5(
            name,
            series_name,
            overview,
            genres,
            content='media_items',
            content_rowid='rowid'
        );

        CREATE TRIGGER IF NOT EXISTS media_items_ai AFTER INSERT ON media_items BEGIN
            INSERT INTO media_items_fts(rowid, name, series_name, overview, genres)
            VALUES (new.rowid, new.name, new.series_name, new.overview, new.genres);
        END;

        CREATE TRIGGER IF NOT EXISTS media_items_ad AFTER DELETE ON media_items BEGIN
            INSERT INTO media_items_fts(media_items_fts, rowid, name, series_name, overview, genres)
            VALUES ('delete', old.rowid, old.name, old.series_name, old.overview, old.genres);
        END;

        CREATE TRIGGER IF NOT EXISTS media_items_au AFTER UPDATE ON media_items BEGIN
            INSERT INTO media_items_fts(media_items_fts, rowid, name, series_name, overview, genres)
            VALUES ('delete', old.rowid, old.name, old.series_name, old.overview, old.genres);
            INSERT INTO media_items_fts(rowid, name, series_name, overview, genres)
            VALUES (new.rowid, new.name, new.series_name, new.overview, new.genres);
        END;

        UPDATE metadata SET value = '3' WHERE key = 'schema_version';",
    )?;

    println!("Database migrated to v3 (media_items + FTS5)");
    Ok(())
}

fn migrate_v3_to_v4(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_checkpoints (
            view_id    TEXT PRIMARY KEY,
            status     TEXT NOT NULL,
            last_index INTEGER NOT NULL
        );

        UPDATE metadata SET value = '4' WHERE key = 'schema_version';",
    )?;

    println!("Database migrated to v4 (sync_checkpoints)");
    Ok(())
}
