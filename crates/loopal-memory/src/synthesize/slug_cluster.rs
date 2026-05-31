use std::collections::HashMap;

use rusqlite::Connection;

use loopal_error::MemoryGraphError;

use crate::store::queries_edge;
use crate::store::queries_node;
use crate::store::types::{EdgeKind, MemoryEdge, Provenance};

const MIN_CLUSTER_SIZE: usize = 2;

pub fn synthesize_sync(conn: &Connection, now: i64) -> Result<usize, MemoryGraphError> {
    let nodes = queries_node::list(conn, None)?;
    let clusters = group_by_prefix(&nodes.iter().map(|n| n.id.clone()).collect::<Vec<_>>());

    let mut added = 0usize;
    for (_prefix, ids) in clusters {
        if ids.len() < MIN_CLUSTER_SIZE {
            continue;
        }
        // confidence 自然衰减 1/(N-1)，大簇 (N≥5) 自动 <0.3 被默认 BFS min_confidence
        // 过滤，避免跨主题噪声；小簇 (N=2..4) 保留紧密语义边。anchor mode 需要保留
        // 大簇内的连接性，靠 RecallParams.min_confidence 单独下调（见 recall.rs）。
        let conf = if ids.len() == 1 {
            1.0
        } else {
            1.0 / (ids.len() - 1) as f32
        };
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let edge = MemoryEdge {
                    id: None,
                    src_id: ids[i].clone(),
                    dst_id: ids[j].clone(),
                    kind: EdgeKind::CoOccursSlug,
                    line: None,
                    metadata: Some(serde_json::json!({ "synthesizer": "slug_cluster" })),
                    provenance: Provenance::Synthesized,
                    confidence: conf,
                    created_at: now,
                };
                queries_edge::insert(conn, &edge)?;
                added += 1;
            }
        }
    }
    Ok(added)
}

pub fn group_by_prefix(ids: &[String]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for id in ids {
        if let Some(prefix) = first_prefix_segment(id) {
            map.entry(prefix).or_default().push(id.clone());
        }
    }
    map.retain(|_, v| v.len() >= MIN_CLUSTER_SIZE);
    map
}

fn first_prefix_segment(id: &str) -> Option<String> {
    let s = id.trim();
    let end = s.find('-')?;
    if end == 0 {
        return None;
    }
    Some(s[..end].to_string())
}
