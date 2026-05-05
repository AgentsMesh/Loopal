use loopal_tool_api::backend_types::ExecResult;
use loopal_tool_api::{
    DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_OUTPUT_LINES, needs_truncation, truncate_middle,
    truncate_tail,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationStrategy {
    Default,
    TailHeavy,
    StackTrace,
    DiffByFile,
}

pub struct StrategyOutcome {
    pub stdout: String,
    pub stderr: String,
    pub applied: Option<&'static str>,
    pub hint: Option<&'static str>,
}

pub fn detect_strategy(exec: &ExecResult, command: &str) -> TruncationStrategy {
    if has_panic(&exec.stdout) || has_panic(&exec.stderr) {
        return TruncationStrategy::StackTrace;
    }
    if has_diff_marker(&exec.stdout) {
        return TruncationStrategy::DiffByFile;
    }
    if is_log_command(command) {
        return TruncationStrategy::TailHeavy;
    }
    TruncationStrategy::Default
}

pub fn apply(strategy: TruncationStrategy, exec: &ExecResult) -> StrategyOutcome {
    match strategy {
        TruncationStrategy::Default => apply_default(exec),
        TruncationStrategy::TailHeavy => apply_tail_heavy(exec),
        TruncationStrategy::StackTrace => apply_stack_trace(exec),
        TruncationStrategy::DiffByFile => apply_diff_by_file(exec),
    }
}

fn has_panic(s: &str) -> bool {
    s.lines()
        .any(|l| l.starts_with("panic:") || (l.starts_with("thread '") && l.contains("' panicked")))
}

fn has_diff_marker(s: &str) -> bool {
    s.lines()
        .any(|l| l.starts_with("diff --git a/") && l.contains(" b/"))
}

fn is_log_command(command: &str) -> bool {
    let trimmed = command.trim_start();
    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    matches!(first_word, "tail" | "journalctl")
        || trimmed.starts_with("docker logs")
        || trimmed.starts_with("kubectl logs")
}

fn apply_default(exec: &ExecResult) -> StrategyOutcome {
    StrategyOutcome {
        stdout: truncate_middle(
            &exec.stdout,
            DEFAULT_MAX_OUTPUT_LINES,
            DEFAULT_MAX_OUTPUT_BYTES,
            60,
        ),
        stderr: truncate_middle(
            &exec.stderr,
            DEFAULT_MAX_OUTPUT_LINES,
            DEFAULT_MAX_OUTPUT_BYTES,
            50,
        ),
        applied: None,
        hint: None,
    }
}

fn apply_tail_heavy(exec: &ExecResult) -> StrategyOutcome {
    StrategyOutcome {
        stdout: truncate_tail(
            &exec.stdout,
            DEFAULT_MAX_OUTPUT_LINES,
            DEFAULT_MAX_OUTPUT_BYTES,
        ),
        stderr: truncate_tail(
            &exec.stderr,
            DEFAULT_MAX_OUTPUT_LINES,
            DEFAULT_MAX_OUTPUT_BYTES,
        ),
        applied: Some("tail_heavy_strategy"),
        hint: Some("log tail detected — only the last lines retained"),
    }
}

fn apply_stack_trace(exec: &ExecResult) -> StrategyOutcome {
    StrategyOutcome {
        stdout: keep_from_panic(&exec.stdout),
        stderr: keep_from_panic(&exec.stderr),
        applied: Some("stack_trace_strategy"),
        hint: Some("panic detected — output retained from panic line onward"),
    }
}

const PANIC_CONTEXT_LINES: usize = 5;

fn keep_from_panic(s: &str) -> String {
    if !needs_truncation(s, DEFAULT_MAX_OUTPUT_LINES, DEFAULT_MAX_OUTPUT_BYTES) {
        return s.to_string();
    }
    let lines: Vec<&str> = s.lines().collect();
    let panic_idx = lines.iter().position(|l| {
        l.starts_with("panic:") || (l.starts_with("thread '") && l.contains("' panicked"))
    });
    let Some(idx) = panic_idx else {
        return truncate_tail(s, DEFAULT_MAX_OUTPUT_LINES, DEFAULT_MAX_OUTPUT_BYTES);
    };
    let start = idx.saturating_sub(PANIC_CONTEXT_LINES);
    let preserved = lines[start..].join("\n");
    let body = truncate_middle(
        &preserved,
        DEFAULT_MAX_OUTPUT_LINES,
        DEFAULT_MAX_OUTPUT_BYTES,
        10,
    );
    if start == 0 {
        return body;
    }
    let dropped_lines = start;
    let dropped_bytes = lines[..start].iter().map(|l| l.len() + 1).sum::<usize>();
    format!(
        "[head truncated: {dropped_lines} lines, {dropped_bytes} bytes omitted before panic context]\n{body}"
    )
}

fn apply_diff_by_file(exec: &ExecResult) -> StrategyOutcome {
    StrategyOutcome {
        stdout: condense_diff(&exec.stdout),
        stderr: truncate_middle(
            &exec.stderr,
            DEFAULT_MAX_OUTPUT_LINES,
            DEFAULT_MAX_OUTPUT_BYTES,
            50,
        ),
        applied: Some("diff_by_file_strategy"),
        hint: Some(
            "large diff — use 'git diff --stat' first, then 'git diff -- <path>' for specific files",
        ),
    }
}

fn condense_diff(s: &str) -> String {
    if !needs_truncation(s, DEFAULT_MAX_OUTPUT_LINES, DEFAULT_MAX_OUTPUT_BYTES) {
        return s.to_string();
    }
    let mut out = String::new();
    let mut hunk_lines_emitted = 0usize;
    let mut in_hunk = false;
    for line in s.lines() {
        if line.starts_with("diff --git ")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
        {
            in_hunk = false;
            hunk_lines_emitted = 0;
            out.push_str(line);
            out.push('\n');
        } else if line.starts_with("@@") {
            in_hunk = true;
            hunk_lines_emitted = 0;
            out.push_str(line);
            out.push('\n');
        } else if in_hunk && hunk_lines_emitted < 5 {
            out.push_str(line);
            out.push('\n');
            hunk_lines_emitted += 1;
        }
    }
    out.trim_end().to_string()
}
