use rusqlite::Connection;

use loopal_error::MemoryGraphError;

use crate::store::queries_node;
use crate::store::types::{EdgeKind, MemoryNode};

pub fn high_impact(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<(MemoryNode, usize)>, MemoryGraphError> {
    let mut stmt = conn.prepare(
        "SELECT dst_id, count(*) AS c
         FROM memory_edges
         WHERE provenance != 'synthesized'
         GROUP BY dst_id
         ORDER BY c DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit as i64], |row| {
        let id: String = row.get(0)?;
        let c: i64 = row.get(1)?;
        Ok((id, c as usize))
    })?;

    let pairs: Vec<(String, usize)> = rows.collect::<Result<Vec<_>, _>>()?;
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<String> = pairs.iter().map(|(i, _)| i.clone()).collect();
    let nodes = queries_node::get_many(conn, &ids)?;
    let mut by_id: std::collections::HashMap<String, MemoryNode> =
        nodes.into_iter().map(|n| (n.id.clone(), n)).collect();

    Ok(pairs
        .into_iter()
        .filter_map(|(id, c)| by_id.remove(&id).map(|n| (n, c)))
        .collect())
}

pub fn expired(conn: &Connection, now_ms: i64) -> Result<Vec<MemoryNode>, MemoryGraphError> {
    let mut stmt = conn.prepare(
        "SELECT id FROM memory_nodes
         WHERE ttl_days IS NOT NULL
           AND (created_at + ttl_days * 86400000) < ?1",
    )?;
    let ids: Vec<String> = stmt
        .query_map([now_ms], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    queries_node::get_many(conn, &ids)
}

pub fn conflicting_pairs(conn: &Connection) -> Result<Vec<(String, String)>, MemoryGraphError> {
    let mut stmt = conn.prepare("SELECT src_id, dst_id FROM memory_edges WHERE kind = ?1")?;
    let rows = stmt.query_map([EdgeKind::Contradicts.as_str()], |row| {
        let s: String = row.get(0)?;
        let d: String = row.get(1)?;
        Ok((s, d))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(MemoryGraphError::Sqlite)
}
