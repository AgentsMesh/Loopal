use rusqlite::Connection;

use loopal_error::MemoryGraphError;

use crate::store::queries_edge;
use crate::store::queries_node;
use crate::store::types::{EdgeKind, MemoryEdge, Provenance};
use crate::synthesize::tfidf::{
    cosine_similarity, document_frequency, term_frequency, tf_idf_vector, tokenize,
};

const SIMILARITY_THRESHOLD: f32 = 0.3;
const MAX_NODES_FOR_TOKEN_CLUSTER: usize = 800;

pub fn synthesize_sync(conn: &Connection, now: i64) -> Result<usize, MemoryGraphError> {
    let nodes = queries_node::list(conn, None)?;
    if nodes.len() < 2 {
        return Ok(0);
    }
    if nodes.len() > MAX_NODES_FOR_TOKEN_CLUSTER {
        return Ok(0);
    }

    let per_doc_tokens: Vec<Vec<String>> = nodes
        .iter()
        .map(|n| tokenize(&format!("{} {}", n.name, n.body_preview)))
        .collect();
    let df = document_frequency(&per_doc_tokens);
    let vectors: Vec<_> = per_doc_tokens
        .iter()
        .map(|tokens| {
            let tf = term_frequency(tokens);
            tf_idf_vector(&tf, &df, nodes.len())
        })
        .collect();

    let mut added = 0usize;
    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            let sim = cosine_similarity(&vectors[i], &vectors[j]);
            if sim < SIMILARITY_THRESHOLD {
                continue;
            }
            let edge = MemoryEdge {
                id: None,
                src_id: nodes[i].id.clone(),
                dst_id: nodes[j].id.clone(),
                kind: EdgeKind::CoOccursToken,
                line: None,
                metadata: Some(serde_json::json!({ "synthesizer": "token_cluster" })),
                provenance: Provenance::Synthesized,
                confidence: sim,
                created_at: now,
            };
            queries_edge::insert(conn, &edge)?;
            added += 1;
        }
    }
    Ok(added)
}
