use rusqlite::{Connection, params};

use loopal_error::MemoryGraphError;

pub struct FileCacheEntry {
    pub modified_at: i64,
    pub size: i64,
}

pub fn get(conn: &Connection, path: &str) -> Result<Option<FileCacheEntry>, MemoryGraphError> {
    let result = conn
        .query_row(
            "SELECT modified_at, size FROM memory_files WHERE path = ?1",
            params![path],
            |row| {
                Ok(FileCacheEntry {
                    modified_at: row.get(0)?,
                    size: row.get(1)?,
                })
            },
        )
        .map(Some);
    match result {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn upsert(
    conn: &Connection,
    path: &str,
    content_hash: &str,
    size: i64,
    modified_at: i64,
    indexed_at: i64,
) -> Result<(), MemoryGraphError> {
    conn.execute(
        "INSERT INTO memory_files (path, content_hash, size, modified_at, indexed_at, errors)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL)
         ON CONFLICT(path) DO UPDATE SET
            content_hash = excluded.content_hash,
            size = excluded.size,
            modified_at = excluded.modified_at,
            indexed_at = excluded.indexed_at,
            errors = NULL",
        params![path, content_hash, size, modified_at, indexed_at],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, path: &str) -> Result<bool, MemoryGraphError> {
    let n = conn.execute("DELETE FROM memory_files WHERE path = ?1", params![path])?;
    Ok(n > 0)
}
