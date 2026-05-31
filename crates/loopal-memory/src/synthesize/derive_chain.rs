use std::collections::HashMap;

use rusqlite::Connection;

use loopal_error::MemoryGraphError;

use crate::store::queries_edge;
use crate::store::queries_node;
use crate::store::types::{EdgeKind, MemoryEdge, MemoryKind, Provenance};

pub fn synthesize_sync(conn: &Connection, now: i64) -> Result<usize, MemoryGraphError> {
    let nodes = queries_node::list(conn, None)?;
    let kind_by_id: HashMap<String, MemoryKind> =
        nodes.iter().map(|n| (n.id.clone(), n.kind)).collect();

    let mut added = 0usize;
    for src in nodes.iter().filter(|n| n.kind == MemoryKind::Project) {
        let outgoing = queries_edge::get_outgoing(conn, &src.id)?;
        for e in outgoing {
            if e.kind != EdgeKind::References {
                continue;
            }
            let target_kind = match kind_by_id.get(&e.dst_id) {
                Some(k) => *k,
                None => continue,
            };
            if target_kind != MemoryKind::User {
                continue;
            }
            let edge = MemoryEdge {
                id: None,
                src_id: src.id.clone(),
                dst_id: e.dst_id.clone(),
                kind: EdgeKind::DerivedFrom,
                line: None,
                metadata: Some(serde_json::json!({ "synthesizer": "derive_chain" })),
                provenance: Provenance::Synthesized,
                confidence: 1.0,
                created_at: now,
            };
            queries_edge::insert(conn, &edge)?;
            added += 1;
        }
    }
    Ok(added)
}
