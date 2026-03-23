/// Diff operation types for LCS-based diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Equal(String),
    Delete(String),
    Insert(String),
}

/// Compute diff between two line slices using LCS dynamic programming.
/// Uses u16 DP table; supports up to 2000 lines per input.
pub fn compute_diff(old: &[&str], new: &[&str]) -> Vec<DiffOp> {
    let m = old.len();
    let n = new.len();

    // Build DP table (lengths stored as u16 to save memory)
    let mut dp = vec![vec![0u16; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if old[i - 1] == new[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    // Backtrack to produce diff ops
    let mut ops = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            ops.push(DiffOp::Equal(old[i - 1].to_string()));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push(DiffOp::Insert(new[j - 1].to_string()));
            j -= 1;
        } else {
            ops.push(DiffOp::Delete(old[i - 1].to_string()));
            i -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Format diff ops as unified diff output.
pub fn format_unified(old_name: &str, new_name: &str, ops: &[DiffOp], context: usize) -> String {
    let mut output = format!("--- {old_name}\n+++ {new_name}\n");
    let hunks = build_hunks(ops, context);

    for hunk in hunks {
        let header = format!(
            "@@ -{},{} +{},{} @@\n",
            hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
        );
        output.push_str(&header);
        output.push_str(&hunk.body);
    }
    output
}

struct Hunk {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    body: String,
}

fn build_hunks(ops: &[DiffOp], context: usize) -> Vec<Hunk> {
    // Identify change ranges (indices of non-Equal ops)
    let change_indices: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, op)| !matches!(op, DiffOp::Equal(_)))
        .map(|(i, _)| i)
        .collect();

    if change_indices.is_empty() {
        return Vec::new();
    }

    // Group changes into hunks with context overlap
    let mut groups: Vec<(usize, usize)> = Vec::new(); // (first_change_idx, last_change_idx)
    let mut cur_start = change_indices[0];
    let mut cur_end = change_indices[0];

    for &idx in &change_indices[1..] {
        // If gap between changes <= 2*context, merge into same hunk
        if idx.saturating_sub(cur_end) <= 2 * context + 1 {
            cur_end = idx;
        } else {
            groups.push((cur_start, cur_end));
            cur_start = idx;
            cur_end = idx;
        }
    }
    groups.push((cur_start, cur_end));

    let mut hunks = Vec::new();
    for (g_start, g_end) in groups {
        let hunk_start = g_start.saturating_sub(context);
        let hunk_end = (g_end + context + 1).min(ops.len());

        let mut body = String::new();
        let mut old_line = 1usize;
        let mut new_line = 1usize;
        // Count lines before hunk_start to get starting line numbers
        for op in &ops[..hunk_start] {
            match op {
                DiffOp::Equal(_) => {
                    old_line += 1;
                    new_line += 1;
                }
                DiffOp::Delete(_) => {
                    old_line += 1;
                }
                DiffOp::Insert(_) => {
                    new_line += 1;
                }
            }
        }
        let old_start = old_line;
        let new_start = new_line;
        let (mut old_count, mut new_count) = (0, 0);

        for op in &ops[hunk_start..hunk_end] {
            match op {
                DiffOp::Equal(l) => {
                    body.push_str(&format!(" {l}\n"));
                    old_count += 1;
                    new_count += 1;
                }
                DiffOp::Delete(l) => {
                    body.push_str(&format!("-{l}\n"));
                    old_count += 1;
                }
                DiffOp::Insert(l) => {
                    body.push_str(&format!("+{l}\n"));
                    new_count += 1;
                }
            }
        }
        hunks.push(Hunk {
            old_start,
            old_count,
            new_start,
            new_count,
            body,
        });
    }
    hunks
}
