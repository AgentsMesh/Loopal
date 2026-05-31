pub mod ranking;

use std::fmt::Write;

use chrono::Utc;
use loopal_error::MemoryGraphError;

use crate::render::ranking::{RankedNode, rank};
use crate::store::MemoryGraph;
use crate::store::types::MemoryKind;

const TOP_PER_KIND: usize = 8;
const RECENT_LIMIT: usize = 5;

pub async fn render_memory_md(graph: &MemoryGraph) -> Result<String, MemoryGraphError> {
    let nodes = graph.list_nodes(None).await?;
    let incoming_counts = graph.count_incoming_all().await?;
    let mut ranked: Vec<RankedNode> = Vec::with_capacity(nodes.len());
    for n in nodes {
        if n.kind == MemoryKind::Index {
            continue;
        }
        let incoming = incoming_counts.get(&n.id).copied().unwrap_or(0);
        ranked.push(rank(n, incoming));
    }
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let edge_count = graph.edge_count().await?;
    Ok(format_index(&ranked, edge_count))
}

fn format_index(ranked: &[RankedNode], edge_count: usize) -> String {
    let mut out = String::new();
    write_frontmatter(&mut out, ranked.len(), edge_count);
    out.push_str("# Memory Index\n\n");

    write_section(&mut out, ranked, MemoryKind::Project, "Project");
    write_section(&mut out, ranked, MemoryKind::Feedback, "Feedback");
    write_section(&mut out, ranked, MemoryKind::User, "User");
    write_recent(&mut out, ranked);
    write_section(&mut out, ranked, MemoryKind::Reference, "Reference");

    out.push_str("\n[Use memory_recall(query=...) to look up content. Do NOT Read individual .md files unless you are about to edit one.]\n");
    out
}

fn write_frontmatter(out: &mut String, node_count: usize, edge_count: usize) {
    let ts = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    writeln!(out, "---").ok();
    writeln!(out, "generated_from: graph").ok();
    writeln!(out, "generated_at: {}", ts).ok();
    writeln!(out, "node_count: {}", node_count).ok();
    writeln!(out, "edge_count: {}", edge_count).ok();
    writeln!(out, "---").ok();
    out.push('\n');
}

fn write_section(out: &mut String, ranked: &[RankedNode], kind: MemoryKind, title: &str) {
    let filtered: Vec<&RankedNode> = ranked
        .iter()
        .filter(|r| r.node.kind == kind)
        .take(TOP_PER_KIND)
        .collect();
    if filtered.is_empty() {
        return;
    }
    writeln!(out, "## {} (high-relevance)", title).ok();
    for r in filtered {
        write_entry(out, r);
    }
    out.push('\n');
}

fn write_entry(out: &mut String, r: &RankedNode) {
    let desc = r.node.description.as_deref().unwrap_or("(no description)");
    let inbound_tag = if r.incoming_count == 0 {
        String::new()
    } else {
        format!(" (referenced by {})", r.incoming_count)
    };
    writeln!(out, "- [[{}]]{} — {}", r.node.id, inbound_tag, desc).ok();
}

fn write_recent(out: &mut String, ranked: &[RankedNode]) {
    let mut by_updated: Vec<&RankedNode> = ranked.iter().collect();
    by_updated.sort_by_key(|r| std::cmp::Reverse(r.node.updated_at));
    let recent: Vec<&RankedNode> = by_updated.into_iter().take(RECENT_LIMIT).collect();
    if recent.is_empty() {
        return;
    }
    writeln!(out, "## Recently added").ok();
    for r in recent {
        let date = chrono::DateTime::from_timestamp_millis(r.node.updated_at)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "unknown".into());
        writeln!(out, "- {} [[{}]]", date, r.node.id).ok();
    }
    out.push('\n');
}
