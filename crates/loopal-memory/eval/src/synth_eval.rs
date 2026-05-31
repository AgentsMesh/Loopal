use std::collections::{HashMap, HashSet};

use loopal_memory::EdgeKind;
use loopal_memory::store::types::MemoryEdge;
use loopal_memory::{MemoryGraph, Provenance};

use crate::runner::{QueryReport, SynthReport};

pub async fn score(graph: &MemoryGraph, queries: &[QueryReport]) -> Vec<SynthReport> {
    let edges = graph
        .list_edges_by_provenance(Provenance::Synthesized)
        .await
        .expect("list synth");

    let mut by_kind: HashMap<String, (usize, usize)> = HashMap::new();

    for e in edges.iter() {
        let synth_name = classify(e);
        let entry = by_kind.entry(synth_name).or_insert((0, 0));
        entry.0 += 1;
        if is_plausible(e, queries) {
            entry.1 += 1;
        }
    }

    let mut out: Vec<SynthReport> = by_kind
        .into_iter()
        .map(|(synthesizer, (sampled, plausible))| SynthReport {
            synthesizer,
            sampled,
            plausible,
        })
        .collect();
    out.sort_by(|a, b| a.synthesizer.cmp(&b.synthesizer));
    out
}

fn classify(edge: &MemoryEdge) -> String {
    match edge.kind {
        EdgeKind::DerivedFrom => "derive_chain".into(),
        EdgeKind::SupersededBy => "supersede".into(),
        EdgeKind::CoOccursSlug => "slug_cluster".into(),
        EdgeKind::CoOccursToken => "token_cluster".into(),
        _ => "other".into(),
    }
}

fn is_plausible(edge: &MemoryEdge, queries: &[QueryReport]) -> bool {
    for q in queries {
        let ids: HashSet<&str> = q.spec.relevant.iter().map(|r| r.id.as_str()).collect();
        if ids.contains(edge.src_id.as_str()) && ids.contains(edge.dst_id.as_str()) {
            return true;
        }
    }
    false
}
