use rusqlite::{Connection, params};

use loopal_error::MemoryGraphError;

use crate::store::types::MemoryNode;

pub fn upsert(conn: &Connection, node: &MemoryNode) -> Result<(), MemoryGraphError> {
    let existing_path: Option<String> = conn
        .query_row(
            "SELECT file_path FROM memory_nodes WHERE id = ?1",
            params![node.id],
            |row| row.get(0),
        )
        .ok();
    if let Some(existing) = existing_path
        && existing != node.file_path
    {
        tracing::warn!(
            slug = %node.id,
            existing_file_path = %existing,
            new_file_path = %node.file_path,
            "slug collision: two distinct files produced the same slug — second upsert will overwrite first"
        );
    }

    conn.execute(
        "INSERT INTO memory_nodes (
            id, kind, name, description, file_path, body_preview,
            created_at, updated_at, ttl_days, content_hash, indexed_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(id) DO UPDATE SET
            kind = excluded.kind,
            name = excluded.name,
            description = excluded.description,
            file_path = excluded.file_path,
            body_preview = excluded.body_preview,
            updated_at = CASE
                WHEN content_hash != excluded.content_hash THEN excluded.updated_at
                ELSE updated_at
            END,
            ttl_days = excluded.ttl_days,
            content_hash = excluded.content_hash,
            indexed_at = excluded.indexed_at",
        params![
            node.id,
            node.kind.as_str(),
            node.name,
            node.description,
            node.file_path,
            node.body_preview,
            node.created_at,
            node.updated_at,
            node.ttl_days,
            node.content_hash,
            node.indexed_at,
        ],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> Result<bool, MemoryGraphError> {
    let n = conn.execute("DELETE FROM memory_nodes WHERE id = ?1", params![id])?;
    Ok(n > 0)
}

pub fn rename(
    conn: &Connection,
    old_id: &str,
    new_id: &str,
    new_file_path: &str,
) -> Result<bool, MemoryGraphError> {
    if old_id == new_id {
        let n = conn.execute(
            "UPDATE memory_nodes SET file_path = ?1 WHERE id = ?2",
            params![new_file_path, old_id],
        )?;
        return Ok(n > 0);
    }

    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM memory_nodes WHERE id = ?1",
            params![new_id],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if exists {
        return Ok(false);
    }

    conn.execute("PRAGMA defer_foreign_keys = ON", [])?;
    conn.execute(
        "UPDATE memory_edges SET src_id = ?1 WHERE src_id = ?2",
        params![new_id, old_id],
    )?;
    conn.execute(
        "UPDATE memory_edges SET dst_id = ?1 WHERE dst_id = ?2",
        params![new_id, old_id],
    )?;
    conn.execute(
        "UPDATE memory_unresolved_links SET from_id = ?1 WHERE from_id = ?2",
        params![new_id, old_id],
    )?;
    let n = conn.execute(
        "UPDATE memory_nodes SET id = ?1, file_path = ?2 WHERE id = ?3",
        params![new_id, new_file_path, old_id],
    )?;
    Ok(n > 0)
}
