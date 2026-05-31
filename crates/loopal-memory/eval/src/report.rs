use std::fmt::Write;

use crate::baseline::savings_percent;
use crate::report_retrieved::write_retrieved;
use crate::runner::{EvalReport, QueryReport, SynthReport};

const PASS_R5: f32 = 0.7;
const PASS_R10: f32 = 0.85;
const PASS_MRR: f32 = 0.7;
const PASS_NDCG5: f32 = 0.7;
const PASS_SYNTH: f32 = 0.5;

pub fn render(report: &EvalReport) -> String {
    let mut out = String::new();
    writeln!(out, "# memory_recall Evaluation Report\n").ok();

    write_summary(&mut out, report);
    write_per_query(&mut out, &report.queries);
    write_macro(&mut out, &report.queries);
    write_retrieved(&mut out, &report.queries);
    write_synth(&mut out, &report.synth);
    write_tokens(&mut out, &report.queries);
    write_gates(&mut out, report);

    out
}

fn write_summary(out: &mut String, r: &EvalReport) {
    writeln!(out, "## Dataset").ok();
    writeln!(out, "- Fixtures: {}", r.fixture_count).ok();
    writeln!(
        out,
        "- Total bytes: {:.1} KB",
        r.total_bytes as f32 / 1024.0
    )
    .ok();
    writeln!(out, "- Queries: {}\n", r.queries.len()).ok();
}

fn write_per_query(out: &mut String, qs: &[QueryReport]) {
    writeln!(out, "## Per-query Metrics").ok();
    writeln!(out, "**Primary** (找到相关信息): Recall@K + MRR + nDCG@5").ok();
    writeln!(out, "**Secondary** (精度参考): P@K + F1@5\n").ok();
    writeln!(
        out,
        "| Query | Mode | R@5 | R@10 | MRR | nDCG@5 | P@5 | P@10 | F1@5 |"
    )
    .ok();
    writeln!(out, "|---|---|---|---|---|---|---|---|---|").ok();
    for q in qs {
        writeln!(
            out,
            "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} |",
            q.spec.id,
            q.spec.mode.as_str(),
            q.r_at_5,
            q.r_at_10,
            q.mrr,
            q.ndcg_at_5,
            q.p_at_5,
            q.p_at_10,
            q.f1_at_5,
        )
        .ok();
    }
    out.push('\n');
}

fn write_macro(out: &mut String, qs: &[QueryReport]) {
    let n = qs.len() as f32;
    let mean = |f: fn(&QueryReport) -> f32| -> f32 {
        if n == 0.0 {
            0.0
        } else {
            qs.iter().map(f).sum::<f32>() / n
        }
    };
    writeln!(out, "## Macro Average").ok();
    writeln!(out, "| Metric | Value | Tier |").ok();
    writeln!(out, "|---|---|---|").ok();
    writeln!(out, "| **R@5** | {:.3} | Primary |", mean(|q| q.r_at_5)).ok();
    writeln!(out, "| **R@10** | {:.3} | Primary |", mean(|q| q.r_at_10)).ok();
    writeln!(out, "| **MRR** | {:.3} | Primary |", mean(|q| q.mrr)).ok();
    writeln!(
        out,
        "| **nDCG@5** | {:.3} | Primary |",
        mean(|q| q.ndcg_at_5)
    )
    .ok();
    writeln!(out, "| P@5 | {:.3} | Secondary |", mean(|q| q.p_at_5)).ok();
    writeln!(out, "| P@10 | {:.3} | Secondary |", mean(|q| q.p_at_10)).ok();
    writeln!(out, "| F1@5 | {:.3} | Secondary |", mean(|q| q.f1_at_5)).ok();
    out.push('\n');
}

fn write_synth(out: &mut String, synth: &[SynthReport]) {
    writeln!(out, "## Synthesized Edge Precision").ok();
    writeln!(out, "| Synthesizer | Sampled | Plausible | Precision |").ok();
    writeln!(out, "|---|---|---|---|").ok();
    for s in synth {
        let p = if s.sampled == 0 {
            0.0
        } else {
            s.plausible as f32 / s.sampled as f32
        };
        writeln!(
            out,
            "| {} | {} | {} | {:.3} |",
            s.synthesizer, s.sampled, s.plausible, p,
        )
        .ok();
    }
    out.push('\n');
}

fn write_tokens(out: &mut String, qs: &[QueryReport]) {
    writeln!(out, "## Token Usage (informational, not a gate)").ok();
    writeln!(
        out,
        "fixture 规模小，grep baseline 已很精确；GTM 规模（8KB+/文件）下 recall 优势会显著。\n"
    )
    .ok();
    writeln!(
        out,
        "| Query | read_all | grep_filter | recall | vs read_all | vs grep |"
    )
    .ok();
    writeln!(out, "|---|---|---|---|---|---|").ok();
    for q in qs {
        writeln!(
            out,
            "| {} | {} | {} | {} | {:.0}% | {:.0}% |",
            q.spec.id,
            q.read_all_tokens,
            q.grep_tokens,
            q.recall_tokens,
            savings_percent(q.read_all_tokens, q.recall_tokens),
            savings_percent(q.grep_tokens, q.recall_tokens),
        )
        .ok();
    }
    out.push('\n');
}

fn write_gates(out: &mut String, report: &EvalReport) {
    let n = report.queries.len() as f32;
    let mean_r5: f32 = report.queries.iter().map(|q| q.r_at_5).sum::<f32>() / n;
    let mean_r10: f32 = report.queries.iter().map(|q| q.r_at_10).sum::<f32>() / n;
    let mean_mrr: f32 = report.queries.iter().map(|q| q.mrr).sum::<f32>() / n;
    let mean_ndcg5: f32 = report.queries.iter().map(|q| q.ndcg_at_5).sum::<f32>() / n;
    let synth_strict: f32 = strict_synth_precision(&report.synth);

    writeln!(out, "## Pass/Fail Gates (相关性优先 / 召回为主)").ok();
    gate_line(out, "Macro Recall@5 ≥ 0.70", mean_r5, PASS_R5);
    gate_line(out, "Macro Recall@10 ≥ 0.85", mean_r10, PASS_R10);
    gate_line(out, "Macro MRR ≥ 0.70", mean_mrr, PASS_MRR);
    gate_line(out, "Macro nDCG@5 ≥ 0.70", mean_ndcg5, PASS_NDCG5);
    gate_line(
        out,
        "Synth precision (slug+derive+supersede) ≥ 0.50",
        synth_strict,
        PASS_SYNTH,
    );
}

fn gate_line(out: &mut String, label: &str, value: f32, threshold: f32) {
    let tag = if value >= threshold { "PASS" } else { "FAIL" };
    writeln!(out, "- [{}] {}: {:.3}", tag, label, value).ok();
}

fn strict_synth_precision(synth: &[SynthReport]) -> f32 {
    let mut sampled = 0usize;
    let mut plausible = 0usize;
    for s in synth {
        if matches!(
            s.synthesizer.as_str(),
            "slug_cluster" | "derive_chain" | "supersede"
        ) {
            sampled += s.sampled;
            plausible += s.plausible;
        }
    }
    if sampled == 0 {
        0.0
    } else {
        plausible as f32 / sampled as f32
    }
}
