use rusqlite::{Connection, params};

use loopal_error::MemoryGraphError;

use crate::store::queries_node::get_many;
use crate::store::types::{MemoryKind, MemoryNode};

pub struct FtsHit {
    pub node: MemoryNode,
    pub rank: f32,
}

pub fn search(
    conn: &Connection,
    query: &str,
    kind: Option<MemoryKind>,
    limit: usize,
) -> Result<Vec<FtsHit>, MemoryGraphError> {
    let sanitized = sanitize_query(query);
    if sanitized.is_empty() {
        return Ok(Vec::new());
    }

    let kind_filter = kind.map(|k| k.as_str());

    let sql = if kind_filter.is_some() {
        "SELECT memory_fts.id, bm25(memory_fts) AS rank
         FROM memory_fts
         JOIN memory_nodes n ON memory_fts.id = n.id
         WHERE memory_fts MATCH ?1 AND n.kind = ?2
         ORDER BY rank
         LIMIT ?3"
    } else {
        "SELECT memory_fts.id, bm25(memory_fts) AS rank
         FROM memory_fts
         WHERE memory_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows: Vec<(String, f32)> = if let Some(k) = kind_filter {
        stmt.query_map(params![sanitized, k, limit as i64], |r| {
            let id: String = r.get(0)?;
            let rank: f64 = r.get(1)?;
            Ok((id, rank as f32))
        })?
        .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![sanitized, limit as i64], |r| {
            let id: String = r.get(0)?;
            let rank: f64 = r.get(1)?;
            Ok((id, rank as f32))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<String> = rows.iter().map(|(id, _)| id.clone()).collect();
    let nodes = get_many(conn, &ids)?;

    let mut by_id: std::collections::HashMap<String, MemoryNode> =
        nodes.into_iter().map(|n| (n.id.clone(), n)).collect();

    let mut hits = Vec::with_capacity(rows.len());
    for (id, rank) in rows {
        if let Some(node) = by_id.remove(&id) {
            hits.push(FtsHit { node, rank });
        }
    }
    Ok(hits)
}

fn sanitize_query(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // reason: FTS5 token 用引号包成 phrase 转义操作符；去引号后变空串的 token 必须丢弃
    //  否则会生成 `""` 触发 FTS5 syntax error。
    let tokens: Vec<String> = trimmed
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| t.replace('"', ""))
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t))
        .collect();
    tokens.join(" OR ")
}
