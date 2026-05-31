use std::fmt::Write;

use crate::runner::QueryReport;

pub fn write_retrieved(out: &mut String, qs: &[QueryReport]) {
    writeln!(out, "## Per-query Retrieved (top 12)").ok();
    writeln!(
        out,
        "`✓` = ground-truth relevant；`✗` = noise；尾部 `[missed]` 是该 query 期望但没召回到的 slug\n"
    )
    .ok();
    for q in qs {
        let rel = q.spec.relevant_ids();
        let input = q.spec.query.clone().unwrap_or_default();
        let anchors = if q.spec.anchors.is_empty() {
            String::new()
        } else {
            format!(" anchors={:?}", q.spec.anchors)
        };
        writeln!(
            out,
            "### {} [{}] — `{}`{}\n",
            q.spec.id,
            q.spec.mode.as_str(),
            input,
            anchors
        )
        .ok();
        writeln!(out, "_{}_\n", q.spec.description).ok();
        for (i, slug) in q.retrieved.iter().take(12).enumerate() {
            let tag = if rel.contains(slug) { "✓" } else { "✗" };
            writeln!(out, "{}. {} `{}`", i + 1, tag, slug).ok();
        }
        let missed: Vec<&str> = rel
            .iter()
            .filter(|r| !q.retrieved.contains(r))
            .map(|s| s.as_str())
            .collect();
        if !missed.is_empty() {
            writeln!(out, "\n**missed**: {}", missed.join(", ")).ok();
        }
        out.push('\n');
    }
}
