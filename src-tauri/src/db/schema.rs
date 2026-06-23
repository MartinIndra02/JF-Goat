use rusqlite::Connection;
use std::time::Duration;

use crate::error::JfgoatError;
use crate::db::SQLITE_BUSY_TIMEOUT_MS;

pub fn init_db(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;",
    )?;
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    conn.busy_timeout(Duration::from_millis(SQLITE_BUSY_TIMEOUT_MS))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS metadata (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        INSERT OR IGNORE INTO metadata (key, value) VALUES ('schema_version', '1');",
    )?;

    let tx = conn.unchecked_transaction()?;
    migrate(&tx)?;
    tx.commit()?;
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
        migrate_v4_to_v5(conn)?;
        migrate_v5_to_v6(conn)?;
    } else if version == "2" {
        migrate_v2_to_v3(conn)?;
        migrate_v3_to_v4(conn)?;
        migrate_v4_to_v5(conn)?;
        migrate_v5_to_v6(conn)?;
    } else if version == "3" {
        migrate_v3_to_v4(conn)?;
        migrate_v4_to_v5(conn)?;
        migrate_v5_to_v6(conn)?;
    } else if version == "4" {
        migrate_v4_to_v5(conn)?;
        migrate_v5_to_v6(conn)?;
    } else if version == "5" {
        migrate_v5_to_v6(conn)?;
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
        CREATE INDEX IF NOT EXISTS idx_media_episode_season_order
            ON media_items(season_id, index_number)
            WHERE type = 'Episode';

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

fn migrate_v4_to_v5(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_media_episode_season_order
            ON media_items(season_id, index_number)
            WHERE type = 'Episode';

        UPDATE metadata SET value = '5' WHERE key = 'schema_version';",
    )?;

    println!("Database migrated to v5 (season episode index)");
    Ok(())
}

fn migrate_v5_to_v6(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;

        DROP TRIGGER IF EXISTS media_items_ai;
        DROP TRIGGER IF EXISTS media_items_ad;
        DROP TRIGGER IF EXISTS media_items_au;
        DROP TABLE IF EXISTS media_items_fts;

        CREATE TABLE IF NOT EXISTS media_items_v6 (
            id               TEXT NOT NULL,
            name             TEXT NOT NULL,
            type             TEXT NOT NULL,
            parent_id        TEXT,
            series_id        TEXT,
            series_name      TEXT,
            season_id        TEXT,
            season_name      TEXT,
            index_number     INTEGER,
            production_year  INTEGER,
            overview         TEXT,
            image_tag        TEXT,
            backdrop_tag     TEXT,
            date_created     TEXT,
            date_updated     TEXT,
            community_rating REAL,
            official_rating  TEXT,
            genres           TEXT,
            run_time_ticks   INTEGER,
            played           INTEGER NOT NULL DEFAULT 0,
            play_count       INTEGER NOT NULL DEFAULT 0,
            playback_ticks   INTEGER NOT NULL DEFAULT 0,
            is_favorite      INTEGER NOT NULL DEFAULT 0,
            server_id        TEXT NOT NULL,
            user_id          TEXT NOT NULL,
            PRIMARY KEY (id, server_id, user_id)
        );

        INSERT INTO media_items_v6 (
            id, name, type, parent_id, series_id, series_name,
            season_id, season_name, index_number, production_year,
            overview, image_tag, backdrop_tag, date_created, date_updated,
            community_rating, official_rating, genres, run_time_ticks,
            played, play_count, playback_ticks, is_favorite, server_id, user_id
        )
        SELECT
            mi.id, mi.name, mi.type, mi.parent_id, mi.series_id, mi.series_name,
            mi.season_id, mi.season_name, mi.index_number, mi.production_year,
            mi.overview, mi.image_tag, mi.backdrop_tag, mi.date_created, mi.date_updated,
            mi.community_rating, mi.official_rating, mi.genres, mi.run_time_ticks,
            mi.played, mi.play_count, mi.playback_ticks, mi.is_favorite, mi.server_id,
            COALESCE(
                (SELECT s.user_id FROM servers s WHERE s.id = mi.server_id AND s.user_id IS NOT NULL ORDER BY s.connected_at DESC LIMIT 1),
                '__legacy__'
            )
        FROM media_items mi;

        DROP TABLE IF EXISTS media_items;
        ALTER TABLE media_items_v6 RENAME TO media_items;

        CREATE INDEX IF NOT EXISTS idx_media_type ON media_items(type);
        CREATE INDEX IF NOT EXISTS idx_media_parent ON media_items(parent_id);
        CREATE INDEX IF NOT EXISTS idx_media_series ON media_items(series_id);
        CREATE INDEX IF NOT EXISTS idx_media_date_updated ON media_items(date_updated);
        CREATE INDEX IF NOT EXISTS idx_media_episode_season_order
            ON media_items(season_id, index_number)
            WHERE type = 'Episode';
        CREATE INDEX IF NOT EXISTS idx_media_scope_type ON media_items(server_id, user_id, type);
        CREATE INDEX IF NOT EXISTS idx_media_scope_resume ON media_items(server_id, user_id, played, playback_ticks);
        CREATE INDEX IF NOT EXISTS idx_media_scope_item ON media_items(id, server_id, user_id);

        CREATE VIRTUAL TABLE IF NOT EXISTS media_items_fts USING fts5(
            name,
            series_name,
            overview,
            genres,
            content='media_items',
            content_rowid='rowid'
        );

        INSERT INTO media_items_fts(rowid, name, series_name, overview, genres)
        SELECT rowid, name, series_name, overview, genres FROM media_items;

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

        CREATE TABLE IF NOT EXISTS sync_checkpoints_v6 (
            view_id    TEXT NOT NULL,
            status     TEXT NOT NULL,
            last_index INTEGER NOT NULL,
            server_id  TEXT NOT NULL,
            user_id    TEXT NOT NULL,
            PRIMARY KEY (view_id, server_id, user_id)
        );

        INSERT INTO sync_checkpoints_v6 (view_id, status, last_index, server_id, user_id)
        SELECT
            sc.view_id,
            sc.status,
            sc.last_index,
            COALESCE((SELECT id FROM servers WHERE is_active = 1 ORDER BY connected_at DESC LIMIT 1), '__legacy_server__'),
            COALESCE((SELECT user_id FROM servers WHERE is_active = 1 AND user_id IS NOT NULL ORDER BY connected_at DESC LIMIT 1), '__legacy__')
        FROM sync_checkpoints sc;

        DROP TABLE IF EXISTS sync_checkpoints;
        ALTER TABLE sync_checkpoints_v6 RENAME TO sync_checkpoints;
        CREATE INDEX IF NOT EXISTS idx_sync_checkpoints_scope ON sync_checkpoints(server_id, user_id);

        UPDATE metadata SET value = '6' WHERE key = 'schema_version';
        PRAGMA foreign_keys = ON;",
    )?;

    println!("Database migrated to v6 (media/sync scope isolation)");
    Ok(())
}
