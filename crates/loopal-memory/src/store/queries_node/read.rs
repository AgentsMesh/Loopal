use rusqlite::{Connection, params, params_from_iter};

use loopal_error::MemoryGraphError;

use super::map_row;
use crate::store::types::{MemoryKind, MemoryNode};

pub fn get(conn: &Connection, id: &str) -> Result<Option<MemoryNode>, MemoryGraphError> {
    let result = conn
        .query_row(
            "SELECT id, kind, name, description, file_path, body_preview,
                    created_at, updated_at, ttl_days, content_hash, indexed_at
             FROM memory_nodes WHERE id = ?1",
            params![id],
            map_row,
        )
        .map(Some);

    match result {
        Ok(node) => Ok(node),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(MemoryGraphError::Sqlite(e)),
    }
}

pub fn get_many(conn: &Connection, ids: &[String]) -> Result<Vec<MemoryNode>, MemoryGraphError> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (1..=ids.len())
        .map(|i| format!("?{}", i))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, kind, name, description, file_path, body_preview,
                created_at, updated_at, ttl_days, content_hash, indexed_at
         FROM memory_nodes WHERE id IN ({})",
        placeholders
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(ids.iter()), map_row)?;
    let mut nodes = Vec::with_capacity(ids.len());
    for r in rows {
        nodes.push(r?);
    }
    Ok(nodes)
}

pub fn list(conn: &Connection, limit: Option<usize>) -> Result<Vec<MemoryNode>, MemoryGraphError> {
    let lim = limit.map(|n| n as i64).unwrap_or(-1);
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, description, file_path, body_preview,
                created_at, updated_at, ttl_days, content_hash, indexed_at
         FROM memory_nodes ORDER BY updated_at DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![lim], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(MemoryGraphError::Sqlite)
}

pub fn list_by_kind(
    conn: &Connection,
    kind: MemoryKind,
) -> Result<Vec<MemoryNode>, MemoryGraphError> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, name, description, file_path, body_preview,
                created_at, updated_at, ttl_days, content_hash, indexed_at
         FROM memory_nodes WHERE kind = ?1 ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map(params![kind.as_str()], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(MemoryGraphError::Sqlite)
}

pub fn find_by_file_path(
    conn: &Connection,
    file_path: &str,
) -> Result<Option<MemoryNode>, MemoryGraphError> {
    let result = conn
        .query_row(
            "SELECT id, kind, name, description, file_path, body_preview,
                    created_at, updated_at, ttl_days, content_hash, indexed_at
             FROM memory_nodes WHERE file_path = ?1",
            params![file_path],
            map_row,
        )
        .map(Some);

    match result {
        Ok(node) => Ok(node),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(MemoryGraphError::Sqlite(e)),
    }
}

pub fn count(conn: &Connection) -> Result<usize, MemoryGraphError> {
    let n: i64 = conn.query_row("SELECT count(*) FROM memory_nodes", [], |r| r.get(0))?;
    Ok(n as usize)
}
