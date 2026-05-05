use loopal_tool_api::backend_types::ExecResult;
use loopal_tool_bash::format::format_exec_result;

fn er(stdout: &str, stderr: &str, exit_code: i32) -> ExecResult {
    ExecResult {
        stdout: stdout.into(),
        stderr: stderr.into(),
        exit_code,
    }
}

#[test]
fn format_no_metadata_for_small_success() {
    let result = format_exec_result(er("hi", "", 0), "echo hi");
    assert!(!result.is_error);
    assert_eq!(result.content.trim_end(), "hi");
    assert!(!result.content.contains("exit_code:"));
    assert!(!result.content.contains("stdout_size:"));
}

#[test]
fn format_metadata_present_on_failure() {
    let result = format_exec_result(er("", "", 42), "exit 42");
    assert!(result.is_error);
    assert!(result.content.contains("exit_code: 42"));
}

#[test]
fn format_metadata_present_on_large_success() {
    let big = "x".repeat(2000);
    let result = format_exec_result(er(&big, "", 0), "yes | head -2000");
    assert!(result.content.contains("exit_code: 0"));
    assert!(result.content.contains("stdout_size:"));
}

#[test]
fn format_stderr_section_appears_before_stdout() {
    let result = format_exec_result(
        er("stdout content", "stderr content", 1),
        "echo stuff; exit 1",
    );
    assert!(result.content.contains("--- stderr ---"));
    assert!(result.content.contains("--- stdout ---"));
    let stderr_idx = result.content.find("--- stderr ---").unwrap();
    let stdout_idx = result.content.find("--- stdout ---").unwrap();
    assert!(stderr_idx < stdout_idx);
}

#[test]
fn format_humanizes_size_bytes_kb_mb() {
    let result_b = format_exec_result(er(&"a".repeat(500), "", 1), "");
    assert!(result_b.content.contains("bytes"));

    let result_kb = format_exec_result(er(&"b".repeat(2000), "", 1), "");
    assert!(result_kb.content.contains("KB"));
}

#[test]
fn format_strategy_marker_when_strategy_applied() {
    let mut stdout = String::new();
    for i in 0..3000 {
        stdout.push_str(&format!("log line {i}\n"));
    }
    let result = format_exec_result(er(&stdout, "", 0), "tail -n 5000 /var/log/x.log");
    assert!(result.content.contains("applied: tail_heavy_strategy"));
    assert!(result.content.contains("hint:"));
}

#[test]
fn format_no_strategy_marker_for_default() {
    let mut stdout = String::new();
    for i in 0..5000 {
        stdout.push_str(&format!("line {i}\n"));
    }
    let result = format_exec_result(er(&stdout, "", 0), "some-tool --print");
    assert!(!result.content.contains("applied:"));
    assert!(result.content.contains("[middle truncated:"));
}

#[test]
fn format_extracts_overflow_path_from_backend_footer() {
    let preview = "preview content";
    let path = "/tmp/loopal/overflow/bash_stdout_999.txt";
    let stdout_with_footer = format!(
        "{preview}\n\n\
         [Output too large for context (5.0 MB). Full output saved to: {path}]\n\
         Use the Read tool to access the complete output if needed."
    );
    let result = format_exec_result(er(&stdout_with_footer, "", 1), "yes");
    assert!(result.content.contains("stdout_overflow:"));
    assert!(result.content.contains(path));
}

#[test]
fn format_no_double_overflow_footer() {
    let preview = "preview";
    let path = "/tmp/loopal/overflow/bash_stdout_1.txt";
    let stdout_with_footer = format!(
        "{preview}\n\n\
         [Output too large for context (1.0 MB). Full output saved to: {path}]\n\
         Use the Read tool to access the complete output if needed."
    );
    let result = format_exec_result(er(&stdout_with_footer, "", 1), "yes");
    let occurrences = result.content.matches("Use the Read tool").count();
    assert_eq!(occurrences, 0, "footer should be stripped, not duplicated");
}

#[test]
fn format_hint_appears_when_strategy_provides_one() {
    let mut stdout = String::new();
    for f in 0..100 {
        stdout.push_str(&format!("diff --git a/f{f}.rs b/f{f}.rs\n"));
        stdout.push_str("index 1..2 100644\n--- a/x\n+++ b/x\n@@ -1,30 +1,30 @@\n");
        for h in 0..30 {
            stdout.push_str(&format!(" line {h}\n"));
        }
    }
    let result = format_exec_result(er(&stdout, "", 0), "git diff");
    assert!(result.content.contains("applied: diff_by_file_strategy"));
    assert!(result.content.contains("hint:"));
    assert!(result.content.contains("git diff --stat"));
}
