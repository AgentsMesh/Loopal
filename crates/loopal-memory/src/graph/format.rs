use std::collections::HashMap;
use std::fmt::Write;

use crate::graph::recall::RecallResult;
use crate::store::types::{MemoryEdge, MemoryKind, MemoryNode};

pub fn format_recall(result: &RecallResult) -> String {
    let mut out = String::new();

    write_direct_hits(&mut out, &result.direct_hits);
    write_neighbors(&mut out, result);
    write_co_occurring(&mut out, result);
    write_trail(&mut out, &result.trail, result);

    if out.trim().is_empty() {
        return "No matching memories found. Consider widening the query or adjusting min_confidence.\n".into();
    }

    out.push_str("\n[Use memory_recall again with different query or anchor_names. Avoid Read on individual .loopal/memory/*.md files unless you must edit one.]\n");
    out
}

fn write_direct_hits(out: &mut String, hits: &[crate::graph::recall::ScoredNode]) {
    if hits.is_empty() {
        return;
    }
    out.push_str("## Direct hits\n\n");
    for hit in hits {
        write_node_header(out, &hit.node);
        if !hit.node.body_preview.trim().is_empty() {
            out.push_str(hit.node.body_preview.trim());
            out.push_str("\n\n");
        }
    }
}

fn write_neighbors(out: &mut String, result: &RecallResult) {
    if result.neighbors.is_empty() {
        return;
    }
    out.push_str("## Related (1-2 hop, bidirectional)\n\n");
    for n in &result.neighbors {
        let direction = describe_direction(&n.node.id, &result.trail);
        writeln!(
            out,
            "- {} ({}) — distance {} — {}",
            n.node.id,
            kind_label(n.node.kind),
            n.distance,
            direction
        )
        .ok();
    }
    out.push('\n');
}

fn write_co_occurring(out: &mut String, result: &RecallResult) {
    if result.co_occurring.is_empty() {
        return;
    }
    out.push_str("## Co-occurring (synthesized)\n\n");
    for c in &result.co_occurring {
        writeln!(
            out,
            "- {} ({:.2}) — via {}",
            c.node.id, c.weight, c.via_anchor
        )
        .ok();
    }
    out.push('\n');
}

fn write_trail(out: &mut String, edges: &[MemoryEdge], result: &RecallResult) {
    if edges.is_empty() {
        return;
    }
    let anchor_ids: std::collections::HashSet<&str> = result
        .direct_hits
        .iter()
        .map(|d| d.node.id.as_str())
        .collect();
    out.push_str("## Trail\n\n");
    for e in edges {
        let inbound =
            anchor_ids.contains(e.dst_id.as_str()) && !anchor_ids.contains(e.src_id.as_str());
        let arrow = if inbound { "←" } else { "→" };
        let prov = if e.provenance == crate::store::types::Provenance::Synthesized {
            "synth"
        } else {
            "explicit"
        };
        writeln!(
            out,
            "{} {} [{}, {}, conf {:.2}] {} {}",
            e.src_id,
            arrow,
            e.kind.as_str(),
            prov,
            e.confidence,
            arrow,
            e.dst_id
        )
        .ok();
    }
}

fn write_node_header(out: &mut String, n: &MemoryNode) {
    writeln!(out, "### {} ({})", n.id, kind_label(n.kind)).ok();
    if let Some(d) = &n.description {
        writeln!(out, "> {}", d).ok();
    }
    writeln!(out, "- file: {}", n.file_path).ok();
    writeln!(out, "- updated: {}", format_date(n.updated_at)).ok();
    if let Some(ttl) = n.ttl_days {
        writeln!(out, "- ttl: {}d", ttl).ok();
    }
    out.push('\n');
}

fn kind_label(k: MemoryKind) -> &'static str {
    k.as_str()
}

fn describe_direction(id: &str, edges: &[MemoryEdge]) -> String {
    let mut counts: HashMap<&str, u32> = HashMap::new();
    for e in edges {
        if e.dst_id == id || e.src_id == id {
            *counts.entry(e.kind.as_str()).or_insert(0) += 1;
        }
    }
    let mut parts: Vec<String> = counts
        .into_iter()
        .map(|(k, v)| format!("{}×{}", k, v))
        .collect();
    parts.sort();
    parts.join(", ")
}

fn format_date(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".into())
}
