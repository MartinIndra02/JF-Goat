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
