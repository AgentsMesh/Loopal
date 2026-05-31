use std::sync::LazyLock;

use rusqlite::Connection;

use loopal_error::MemoryGraphError;
use regex::Regex;

use crate::store::queries_edge;
use crate::store::queries_node;
use crate::store::types::{EdgeKind, MemoryEdge, Provenance};

static SUPERSEDE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i:supersedes|replaces|deprecates|取代|废弃)\s*\[\[((?-i:[a-z][a-z0-9_-]*))\]\]")
        .unwrap()
});

pub fn synthesize_sync(conn: &Connection, now: i64) -> Result<usize, MemoryGraphError> {
    let nodes = queries_node::list(conn, None)?;
    let known_ids: std::collections::HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut added = 0usize;
    for n in &nodes {
        for cap in SUPERSEDE_RE.captures_iter(&n.body_preview) {
            let target = match cap.get(1) {
                Some(m) => m.as_str().to_string(),
                None => continue,
            };
            if target == n.id {
                continue;
            }
            if !known_ids.contains(&target) {
                continue;
            }
            let edge = MemoryEdge {
                id: None,
                src_id: target,
                dst_id: n.id.clone(),
                kind: EdgeKind::SupersededBy,
                line: None,
                metadata: Some(serde_json::json!({ "synthesizer": "supersede" })),
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
