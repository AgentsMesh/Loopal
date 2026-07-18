use serde_json::json;

use crate::support::CliHarness;

/// The filesystem tool chain through the wire: Write creates a file, Read
/// registers it (edit precondition), Edit mutates it — then the turn-end diff
/// tracker reports the modified file. Content is verified on disk.
#[tokio::test]
async fn write_read_edit_round_trip_with_diff_summary() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "fs_tools",
        "calls": [
            {"expect": {"userContains": "create the agenda"},
             "chunks": [
                {"type": "tool_use", "id": "w1", "name": "Write",
                 "input": {"file_path": "notes/agenda.txt",
                           "content": "first line\nsecond line\n"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "w1"},
             "chunks": [
                {"type": "tool_use", "id": "r1", "name": "Read",
                 "input": {"file_path": "notes/agenda.txt"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "r1"},
             "chunks": [
                {"type": "tool_use", "id": "e1", "name": "Edit",
                 "input": {"file_path": "notes/agenda.txt",
                           "old_string": "second line",
                           "new_string": "second line, edited"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "e1"},
             "chunks": [{"type": "text", "text": "files done"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.run_turn("please create the agenda").await;
    assert!(
        out.error.is_none() && out.finished && out.text.contains("files done"),
        "turn failed: {out:?}"
    );
    assert_eq!(
        out.tool_result_count(),
        3,
        "Write, Read, Edit must all run; events: {:?}",
        out.events
    );
    assert!(
        !out.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("is_error: true")),
        "no filesystem tool may fail; events: {:?}",
        out.events
    );

    let content = std::fs::read_to_string(h.cwd().join("notes/agenda.txt"))
        .expect("the written file must exist on disk");
    assert_eq!(content, "first line\nsecond line, edited\n");

    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("TurnDiffSummary") && e.contains("agenda.txt")),
        "the diff tracker must report the modified file at turn end; \
         events: {:?}",
        out.events
    );
}

/// Vault safety at the write boundary: a Write whose content carries a wire
/// secret ref must be rejected by the tool's precheck — placeholders never
/// land in user files.
#[tokio::test]
async fn write_rejects_wire_secret_refs() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "fs_wire_ref",
        "calls": [
            {"expect": {"userContains": "write the config"},
             "chunks": [
                {"type": "tool_use", "id": "s1", "name": "Write",
                 "input": {"file_path": "leak.txt",
                           "content": "token=<secret_ref:e2e_token>\n"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "s1"},
             "chunks": [{"type": "text", "text": "rejection handled"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.run_turn("please write the config").await;
    assert!(
        out.finished && out.text.contains("rejection handled"),
        "turn failed: {out:?}"
    );
    assert!(
        out.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("is_error: true")),
        "the wire-ref Write must be rejected; events: {:?}",
        out.events
    );
    assert!(
        !h.cwd().join("leak.txt").exists(),
        "a rejected Write must not create the file"
    );
}

/// Oversized tool output must overflow to a file: the in-context result keeps
/// a preview plus a pointer to the saved full output instead of flooding the
/// conversation. Line-heavy output is already elided by the Bash tool's own
/// head/tail truncation, so the pipeline overflow (100KB) is exercised with a
/// byte-heavy single line.
#[tokio::test]
async fn huge_tool_output_overflows_to_file() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "fs_overflow",
        "calls": [
            {"expect": {"userContains": "dump the blob"},
             "chunks": [
                {"type": "tool_use", "id": "o1", "name": "Bash",
                 "input": {"command": "head -c 150000 /dev/zero | tr '\\0' x"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "o1"},
             "chunks": [{"type": "text", "text": "overflow handled"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.run_turn("please dump the blob").await;
    assert!(
        out.finished && out.text.contains("overflow handled"),
        "turn failed: {out:?}"
    );
    let result = out
        .events
        .iter()
        .find(|e| e.starts_with("ToolResult"))
        .expect("a Bash ToolResult");
    assert!(
        result.contains("Output too large for context") && result.contains("Full output saved to:"),
        "a 150KB single-line output must trigger the overflow-to-file path; \
         result head: {}",
        &result[..result.len().min(400)]
    );
    assert!(
        result.len() < 150_000,
        "the in-context result must be a preview, not the full output \
         (len {})",
        result.len()
    );
}
