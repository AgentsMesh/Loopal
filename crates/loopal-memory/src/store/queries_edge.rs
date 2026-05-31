use rusqlite::{Connection, Row, params};

use loopal_error::MemoryGraphError;

use crate::store::types::{EdgeKind, MemoryEdge, Provenance};

pub fn insert(conn: &Connection, edge: &MemoryEdge) -> Result<i64, MemoryGraphError> {
    let metadata_str = edge.metadata.as_ref().map(|v| v.to_string());

    conn.execute(
        "INSERT INTO memory_edges (src_id, dst_id, kind, line, metadata,
                                    provenance, confidence, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(src_id, dst_id, kind, provenance) DO UPDATE SET
            confidence = max(confidence, excluded.confidence),
            metadata = COALESCE(metadata, excluded.metadata),
            line = COALESCE(line, excluded.line)",
        params![
            edge.src_id,
            edge.dst_id,
            edge.kind.as_str(),
            edge.line,
            metadata_str,
            edge.provenance.as_str(),
            edge.confidence,
            edge.created_at,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_outgoing(conn: &Connection, src_id: &str) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
    query_edges(
        conn,
        "SELECT id, src_id, dst_id, kind, line, metadata, provenance, confidence, created_at
         FROM memory_edges WHERE src_id = ?1",
        params![src_id],
    )
}

pub fn get_incoming(conn: &Connection, dst_id: &str) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
    query_edges(
        conn,
        "SELECT id, src_id, dst_id, kind, line, metadata, provenance, confidence, created_at
         FROM memory_edges WHERE dst_id = ?1",
        params![dst_id],
    )
}

pub fn count_incoming(conn: &Connection, dst_id: &str) -> Result<usize, MemoryGraphError> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM memory_edges WHERE dst_id = ?1",
        params![dst_id],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

pub fn count_incoming_all(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, usize>, MemoryGraphError> {
    let mut stmt = conn.prepare("SELECT dst_id, count(*) FROM memory_edges GROUP BY dst_id")?;
    let rows = stmt.query_map([], |row| {
        let dst: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((dst, count as usize))
    })?;
    rows.collect::<Result<std::collections::HashMap<_, _>, _>>()
        .map_err(MemoryGraphError::Sqlite)
}

pub fn list_by_provenance(
    conn: &Connection,
    provenance: Provenance,
) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
    query_edges(
        conn,
        "SELECT id, src_id, dst_id, kind, line, metadata, provenance, confidence, created_at
         FROM memory_edges WHERE provenance = ?1",
        params![provenance.as_str()],
    )
}

pub fn delete_by_node(conn: &Connection, node_id: &str) -> Result<usize, MemoryGraphError> {
    let n = conn.execute(
        "DELETE FROM memory_edges WHERE src_id = ?1 OR dst_id = ?1",
        params![node_id],
    )?;
    Ok(n)
}

pub fn delete_outgoing(conn: &Connection, src_id: &str) -> Result<usize, MemoryGraphError> {
    let n = conn.execute(
        "DELETE FROM memory_edges WHERE src_id = ?1",
        params![src_id],
    )?;
    Ok(n)
}

pub fn delete_by_provenance(
    conn: &Connection,
    provenance: Provenance,
) -> Result<usize, MemoryGraphError> {
    let n = conn.execute(
        "DELETE FROM memory_edges WHERE provenance = ?1",
        params![provenance.as_str()],
    )?;
    Ok(n)
}

pub fn count(conn: &Connection) -> Result<usize, MemoryGraphError> {
    let n: i64 = conn.query_row("SELECT count(*) FROM memory_edges", [], |r| r.get(0))?;
    Ok(n as usize)
}

fn query_edges(
    conn: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(MemoryGraphError::Sqlite)
}

fn map_row(row: &Row<'_>) -> rusqlite::Result<MemoryEdge> {
    let kind_str: String = row.get("kind")?;
    let prov_str: String = row.get("provenance")?;
    let kind = EdgeKind::parse(&kind_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let provenance = Provenance::parse(&prov_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let metadata_str: Option<String> = row.get("metadata")?;
    let metadata = metadata_str
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
        })?;
    Ok(MemoryEdge {
        id: row.get("id")?,
        src_id: row.get("src_id")?,
        dst_id: row.get("dst_id")?,
        kind,
        line: row.get("line")?,
        metadata,
        provenance,
        confidence: row.get("confidence")?,
        created_at: row.get("created_at")?,
    })
}
