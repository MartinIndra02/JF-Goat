use rusqlite::Connection;

use crate::error::JfgoatError;

pub struct ServerRow {
    pub id: String,
    pub name: String,
    pub url: String,
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub is_active: bool,
}

pub fn upsert_server(
    conn: &Connection,
    id: &str,
    name: &str,
    url: &str,
) -> Result<(), JfgoatError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE servers SET is_active = 0", [])?;

    tx.execute(
        "INSERT INTO servers (id, name, url, is_active, connected_at)
         VALUES (?1, ?2, ?3, 1, datetime('now'))
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            url = excluded.url,
            is_active = 1,
            connected_at = datetime('now')",
        rusqlite::params![id, name, url],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn set_active_server(conn: &Connection, server_id: &str) -> Result<(), JfgoatError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE servers SET is_active = 0", [])?;
    tx.execute(
        "UPDATE servers SET is_active = 1, connected_at = datetime('now') WHERE id = ?1",
        rusqlite::params![server_id],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn update_server_user(
    conn: &Connection,
    server_id: &str,
    user_id: &str,
    username: &str,
) -> Result<(), JfgoatError> {
    conn.execute(
        "UPDATE servers SET user_id = ?1, username = ?2 WHERE id = ?3",
        rusqlite::params![user_id, username, server_id],
    )?;
    Ok(())
}

pub fn get_active_server(conn: &Connection) -> Result<Option<ServerRow>, JfgoatError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, url, user_id, username, is_active
         FROM servers WHERE is_active = 1 ORDER BY connected_at DESC LIMIT 1",
    )?;

    let row = stmt
        .query_row([], |row| {
            Ok(ServerRow {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                user_id: row.get(3)?,
                username: row.get(4)?,
                is_active: row.get::<_, i32>(5)? != 0,
            })
        })
        .optional()?;

    Ok(row)
}

pub fn clear_active_server(conn: &Connection) -> Result<(), JfgoatError> {
    conn.execute("UPDATE servers SET is_active = 0", [])?;
    Ok(())
}

trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
