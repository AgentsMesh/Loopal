use std::collections::{HashMap, HashSet, VecDeque};

use rusqlite::{Connection, params_from_iter};

use loopal_error::MemoryGraphError;

use crate::store::queries_node::get_many;
use crate::store::types::{EdgeKind, MemoryEdge, MemoryNode, Provenance};

#[derive(Debug, Default, Clone)]
pub struct Subgraph {
    pub nodes: Vec<MemoryNode>,
    pub edges: Vec<MemoryEdge>,
    pub depth_by_id: HashMap<String, u32>,
}

pub struct BfsConfig {
    pub max_depth: u32,
    pub edge_kinds: Option<Vec<EdgeKind>>,
    pub exclude_provenance: Vec<Provenance>,
    pub min_confidence: f32,
    pub limit_nodes: usize,
}

impl Default for BfsConfig {
    fn default() -> Self {
        Self {
            max_depth: 2,
            edge_kinds: None,
            exclude_provenance: Vec::new(),
            min_confidence: 0.3,
            limit_nodes: 50,
        }
    }
}

pub fn bfs_bidirectional(
    conn: &Connection,
    start_ids: &[String],
    config: &BfsConfig,
) -> Result<Subgraph, MemoryGraphError> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut depth_by_id: HashMap<String, u32> = HashMap::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut all_edges: Vec<MemoryEdge> = Vec::new();
    let mut seen_edge_ids: HashSet<i64> = HashSet::new();

    for sid in start_ids {
        visited.insert(sid.clone());
        depth_by_id.insert(sid.clone(), 0);
        queue.push_back((sid.clone(), 0));
    }

    while let Some((node_id, depth)) = queue.pop_front() {
        if visited.len() >= config.limit_nodes {
            break;
        }
        if depth >= config.max_depth {
            continue;
        }

        let edges = fetch_edges_for_node(conn, &node_id, config)?;
        for e in edges {
            if let Some(id) = e.id
                && !seen_edge_ids.insert(id)
            {
                continue;
            }
            let neighbor = pick_neighbor(&node_id, &e);
            if !visited.contains(&neighbor) {
                if visited.len() >= config.limit_nodes {
                    all_edges.push(e);
                    continue;
                }
                visited.insert(neighbor.clone());
                depth_by_id.insert(neighbor.clone(), depth + 1);
                queue.push_back((neighbor, depth + 1));
            }
            all_edges.push(e);
        }
    }

    let ids: Vec<String> = visited.into_iter().collect();
    let nodes = get_many(conn, &ids)?;

    Ok(Subgraph {
        nodes,
        edges: all_edges,
        depth_by_id,
    })
}

fn pick_neighbor(self_id: &str, edge: &MemoryEdge) -> String {
    if edge.src_id == self_id {
        edge.dst_id.clone()
    } else {
        edge.src_id.clone()
    }
}

fn fetch_edges_for_node(
    conn: &Connection,
    node_id: &str,
    config: &BfsConfig,
) -> Result<Vec<MemoryEdge>, MemoryGraphError> {
    let mut sql = String::from(
        "SELECT id, src_id, dst_id, kind, line, metadata, provenance, confidence, created_at
         FROM memory_edges
         WHERE (src_id = ?1 OR dst_id = ?1) AND confidence >= ?2",
    );

    let mut bindings: Vec<Box<dyn rusqlite::ToSql>> = vec![
        Box::new(node_id.to_string()),
        Box::new(config.min_confidence as f64),
    ];

    if let Some(kinds) = &config.edge_kinds
        && !kinds.is_empty()
    {
        let next_idx = bindings.len() + 1;
        let placeholders: Vec<String> = (next_idx..next_idx + kinds.len())
            .map(|i| format!("?{}", i))
            .collect();
        sql.push_str(&format!(" AND kind IN ({})", placeholders.join(",")));
        for k in kinds {
            bindings.push(Box::new(k.as_str().to_string()));
        }
    }

    if !config.exclude_provenance.is_empty() {
        let next_idx = bindings.len() + 1;
        let placeholders: Vec<String> = (next_idx..next_idx + config.exclude_provenance.len())
            .map(|i| format!("?{}", i))
            .collect();
        sql.push_str(&format!(
            " AND provenance NOT IN ({})",
            placeholders.join(",")
        ));
        for p in &config.exclude_provenance {
            bindings.push(Box::new(p.as_str().to_string()));
        }
    }

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params_from_iter(bindings.iter().map(|b| b.as_ref())),
        map_edge_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(MemoryGraphError::Sqlite)
}

fn map_edge_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEdge> {
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
