mod baseline;
mod fixture;
mod ground_truth;
mod metrics;
mod report;
mod report_retrieved;
mod runner;
mod synth_eval;

use runner::EvalReport;

fn macro_recall_at_k(report: &EvalReport, k: usize) -> f32 {
    let n = report.queries.len() as f32;
    report
        .queries
        .iter()
        .map(|q| if k == 5 { q.r_at_5 } else { q.r_at_10 })
        .sum::<f32>()
        / n
}

fn macro_mrr(report: &EvalReport) -> f32 {
    let n = report.queries.len() as f32;
    report.queries.iter().map(|q| q.mrr).sum::<f32>() / n
}

fn macro_ndcg(report: &EvalReport) -> f32 {
    let n = report.queries.len() as f32;
    report.queries.iter().map(|q| q.ndcg_at_5).sum::<f32>() / n
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cold = runner::run_with_warmup(0).await;
    let md = report::render(&cold);
    println!("{}", md);

    let warm5 = runner::run_with_warmup(5).await;
    let warm20 = runner::run_with_warmup(20).await;
    let imp5 = runner::run_with_importance(5).await;
    let imp_max = runner::run_with_importance(10).await;
    let per_q5 = runner::run_with_per_query_warmup(5).await;

    println!("\n## Associative Recall A/B\n");
    println!("- **Cold**: no events, no RecallStats");
    println!(
        "- **Warm 5x/20x (global)**: warmup ALL ground-truth relevant — has cross-query pollution"
    );
    println!("- **Imp +5/+10**: install importance on all relevant — cross-query pollution too");
    println!(
        "- **Per-Q 5x**: warmup only current query's relevant (reset stats per query) — clean signal\n"
    );
    println!("| Metric | Cold | Warm 5x | Warm 20x | Imp +5 | Imp +10 | Per-Q 5x |");
    println!("|---|---|---|---|---|---|---|");
    let modes: [&EvalReport; 6] = [&cold, &warm5, &warm20, &imp5, &imp_max, &per_q5];
    print_ab_row("Macro R@5", &modes, |r| macro_recall_at_k(r, 5));
    print_ab_row("Macro R@10", &modes, |r| macro_recall_at_k(r, 10));
    print_ab_row("Macro MRR", &modes, macro_mrr);
    print_ab_row("Macro nDCG@5", &modes, macro_ndcg);

    println!("\n### Per-query MRR (cold vs warm 20x vs per-q 5x)");
    println!("| Query | Cold | Warm 20x | Per-Q 5x | Δ (Per-Q − Cold) |");
    println!("|---|---|---|---|---|");
    for ((c, w), p) in cold
        .queries
        .iter()
        .zip(warm20.queries.iter())
        .zip(per_q5.queries.iter())
    {
        println!(
            "| {} | {:.3} | {:.3} | {:.3} | {:+.3} |",
            c.spec.id,
            c.mrr,
            w.mrr,
            p.mrr,
            p.mrr - c.mrr
        );
    }

    println!("\n## Regression Diagnostics (q51, q10)");
    dump_query_diff("q51", &cold, &warm20);
    dump_query_diff("q51", &cold, &per_q5);
    dump_query_diff("q10", &cold, &per_q5);
}

fn print_ab_row(name: &str, modes: &[&EvalReport], f: impl Fn(&EvalReport) -> f32) {
    print!("| {} |", name);
    for m in modes {
        print!(" {:.3} |", f(m));
    }
    println!();
}

fn dump_query_diff(qid: &str, cold: &EvalReport, warm: &EvalReport) {
    let c = cold.queries.iter().find(|q| q.spec.id == qid);
    let w = warm.queries.iter().find(|q| q.spec.id == qid);
    let (Some(c), Some(w)) = (c, w) else {
        return;
    };
    println!("\n#### {} cold retrieved:", qid);
    for (i, r) in c.retrieved.iter().take(10).enumerate() {
        let rel = c.spec.relevance_map().get(r).copied().unwrap_or(0);
        let mark = if rel > 0 {
            format!("✓ rel={}", rel)
        } else {
            "✗".to_string()
        };
        println!("  {}. `{}` {}", i + 1, r, mark);
    }
    println!("\n#### {} warm 20x retrieved:", qid);
    for (i, r) in w.retrieved.iter().take(10).enumerate() {
        let rel = w.spec.relevance_map().get(r).copied().unwrap_or(0);
        let mark = if rel > 0 {
            format!("✓ rel={}", rel)
        } else {
            "✗".to_string()
        };
        println!("  {}. `{}` {}", i + 1, r, mark);
    }
}
