use loopal_tool_api::backend_types::ExecResult;
use loopal_tool_bash::strategy::{TruncationStrategy, apply, detect_strategy};

fn er(stdout: &str, stderr: &str, exit_code: i32) -> ExecResult {
    ExecResult {
        stdout: stdout.into(),
        stderr: stderr.into(),
        exit_code,
    }
}

#[test]
fn detect_default_when_no_fingerprint_matches() {
    let exec = er("regular output\nline2", "", 0);
    assert_eq!(
        detect_strategy(&exec, "ls -la"),
        TruncationStrategy::Default
    );
}

#[test]
fn detect_default_when_panic_substring_appears_inline() {
    let exec = er(
        "abort note: panic and recovery system\nrebuild started\n",
        "",
        0,
    );
    assert_eq!(detect_strategy(&exec, "make"), TruncationStrategy::Default);
}

#[test]
fn detect_default_when_diff_substring_appears_inline() {
    let exec = er(
        "compared output (diff --git reference format): no actual diff\n",
        "",
        0,
    );
    assert_eq!(
        detect_strategy(&exec, "echo stuff"),
        TruncationStrategy::Default
    );
}

#[test]
fn detect_default_when_command_only_contains_tail_substring() {
    let exec = er("normal output", "", 0);
    assert_eq!(
        detect_strategy(&exec, "echo tailing.rs"),
        TruncationStrategy::Default
    );
    assert_eq!(
        detect_strategy(&exec, "cargo run --bin tailer"),
        TruncationStrategy::Default
    );
}

#[test]
fn detect_default_when_thread_phrase_is_not_panic() {
    let exec = er("thread 'main' started normally\nworking\n", "", 0);
    assert_eq!(
        detect_strategy(&exec, "cargo run"),
        TruncationStrategy::Default
    );
}

#[test]
fn detect_stack_trace_when_panic_in_stdout() {
    let exec = er(
        "running test\nthread 'main' panicked at src/lib.rs:42:5\nstack frame 1\n",
        "",
        101,
    );
    assert_eq!(
        detect_strategy(&exec, "cargo run"),
        TruncationStrategy::StackTrace
    );
}

#[test]
fn detect_stack_trace_when_panic_in_stderr() {
    let exec = er("", "panic: runtime error\nframe 1\n", 1);
    assert_eq!(
        detect_strategy(&exec, "go run main.go"),
        TruncationStrategy::StackTrace
    );
}

#[test]
fn detect_diff_by_file_when_diff_git_marker_present() {
    let exec = er(
        "diff --git a/foo.rs b/foo.rs\nindex abc..def 100644\n--- a/foo.rs\n+++ b/foo.rs\n@@ -1,3 +1,3 @@\n-old\n+new\n",
        "",
        0,
    );
    assert_eq!(
        detect_strategy(&exec, "git diff"),
        TruncationStrategy::DiffByFile
    );
}

#[test]
fn detect_tail_heavy_when_command_starts_with_tail() {
    let exec = er("log line A\nlog line B\n", "", 0);
    assert_eq!(
        detect_strategy(&exec, "tail -n 100 /var/log/x.log"),
        TruncationStrategy::TailHeavy
    );
}

#[test]
fn detect_tail_heavy_when_command_starts_with_journalctl() {
    let exec = er("entry 1\nentry 2\n", "", 0);
    assert_eq!(
        detect_strategy(&exec, "journalctl -u nginx"),
        TruncationStrategy::TailHeavy
    );
}

#[test]
fn detect_tail_heavy_when_command_starts_with_docker_logs() {
    let exec = er("container output\n", "", 0);
    assert_eq!(
        detect_strategy(&exec, "docker logs my-container"),
        TruncationStrategy::TailHeavy
    );
}

#[test]
fn apply_stack_trace_keeps_panic_line_onward() {
    let mut stdout = String::new();
    for i in 0..3000 {
        stdout.push_str(&format!("noise line {i}\n"));
    }
    stdout.push_str("thread 'main' panicked at src/x.rs:1:1\n");
    stdout.push_str("stack frame at src/y.rs:2:2\n");
    let exec = er(&stdout, "", 101);
    let outcome = apply(TruncationStrategy::StackTrace, &exec);
    assert_eq!(outcome.applied, Some("stack_trace_strategy"));
    assert!(outcome.hint.is_some());
    assert!(outcome.stdout.contains("thread 'main' panicked"));
    assert!(outcome.stdout.contains("stack frame at src/y.rs"));
    assert!(outcome.stdout.contains("[head truncated:"));
    assert!(outcome.stdout.contains("before panic context"));
    assert!(
        outcome.stdout.contains("noise line 2999"),
        "should keep last few lines before panic as context"
    );
    assert!(
        outcome.stdout.contains("noise line 2995"),
        "should keep at least 5 lines of pre-panic context"
    );
}

#[test]
fn apply_stack_trace_keeps_pre_panic_context_when_panic_near_top() {
    let stdout = "starting test\nrunning suite\nthread 'main' panicked at src/a.rs:1:1\n"
        .to_string()
        + &"stack frame\n".repeat(3000);
    let exec = er(&stdout, "", 101);
    let outcome = apply(TruncationStrategy::StackTrace, &exec);
    assert!(
        outcome.stdout.starts_with("starting test"),
        "panic at idx<5 → preserve pre-panic context starting from line 0"
    );
    assert!(
        !outcome.stdout.contains("before panic context"),
        "no head-of-file dropped → no 'before panic context' marker"
    );
}

#[test]
fn apply_stack_trace_preserves_tail_frames_for_huge_stack() {
    let mut stdout = String::new();
    stdout.push_str("thread 'main' panicked at src/a.rs:1:1\n");
    for i in 0..5000 {
        stdout.push_str(&format!("   {i}: stack frame at lib.rs:{i}\n"));
    }
    stdout.push_str("note: run with `RUST_BACKTRACE=1`\n");
    let exec = er(&stdout, "", 101);
    let outcome = apply(TruncationStrategy::StackTrace, &exec);
    assert!(outcome.stdout.contains("thread 'main' panicked"));
    assert!(
        outcome.stdout.contains("RUST_BACKTRACE"),
        "tail frames must survive: huge stack should keep the trigger point near the bottom"
    );
    assert!(
        outcome.stdout.contains("[middle truncated:"),
        "huge stack should be middle-truncated, not head-only"
    );
    assert!(
        !outcome.stdout.contains("stack frame at lib.rs:2500"),
        "middle frames must be dropped to make room for the panic head and tail trigger"
    );
}

#[test]
fn apply_diff_by_file_groups_by_file_header() {
    let mut stdout = String::new();
    for f in 0..100 {
        stdout.push_str(&format!("diff --git a/file{f}.rs b/file{f}.rs\n"));
        stdout.push_str("index 111..222 100644\n");
        stdout.push_str(&format!("--- a/file{f}.rs\n"));
        stdout.push_str(&format!("+++ b/file{f}.rs\n"));
        stdout.push_str("@@ -1,30 +1,30 @@\n");
        for h in 0..30 {
            stdout.push_str(&format!(" line {h}\n"));
        }
    }
    let exec = er(&stdout, "", 0);
    let outcome = apply(TruncationStrategy::DiffByFile, &exec);
    assert_eq!(outcome.applied, Some("diff_by_file_strategy"));
    assert!(outcome.stdout.contains("diff --git a/file0.rs"));
    assert!(outcome.stdout.contains("diff --git a/file99.rs"));
    let line_count = outcome.stdout.lines().count();
    assert!(
        line_count < stdout.lines().count() / 2,
        "diff_by_file should significantly condense; got {line_count} lines from {}",
        stdout.lines().count()
    );
}

#[test]
fn apply_tail_heavy_keeps_tail() {
    let mut stdout = String::new();
    for i in 0..3000 {
        stdout.push_str(&format!("log line {i}\n"));
    }
    let exec = er(&stdout, "", 0);
    let outcome = apply(TruncationStrategy::TailHeavy, &exec);
    assert_eq!(outcome.applied, Some("tail_heavy_strategy"));
    assert!(outcome.stdout.starts_with("[head truncated:"));
    assert!(outcome.stdout.contains("log line 2999"));
    assert!(!outcome.stdout.contains("log line 0\n"));
}

#[test]
fn apply_default_uses_truncate_middle() {
    let mut stdout = String::new();
    for i in 0..5000 {
        stdout.push_str(&format!("line {i}\n"));
    }
    let exec = er(&stdout, "", 0);
    let outcome = apply(TruncationStrategy::Default, &exec);
    assert!(outcome.applied.is_none());
    assert!(outcome.hint.is_none());
    assert!(outcome.stdout.contains("[middle truncated:"));
    assert!(outcome.stdout.contains("line 0"));
    assert!(outcome.stdout.contains("line 4999"));
}
