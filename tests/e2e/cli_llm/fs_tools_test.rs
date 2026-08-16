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

/// Regression for the ApplyPatch contract used by model-generated patches.
/// The real `loopal --serve` child receives an enveloped patch over Anthropic
/// HTTP/SSE, executes all operation kinds, reports the canonical paths, and
/// sends the successful ToolResult back over the next provider request.
#[tokio::test]
async fn apply_patch_envelope_round_trips_over_http_and_reports_operation_paths() {
    const PATCH: &str = "\
*** Begin Patch
*** Update File: existing.txt
@@
-old value
+updated value
*** Delete File: obsolete.txt
*** Add File: alpha.txt
+alpha
*** Add File: nested/beta.txt
+beta
*** Add File: file with spaces.txt
+spaces
*** End Patch
";

    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "apply_patch_envelope",
        "calls": [
            {"expect": {"protocol": "anthropic", "userContains": "apply the envelope patch"},
             "chunks": [
                {"type": "tool_use", "id": "ap1", "name": "ApplyPatch",
                 "input": {"patch": PATCH}},
                {"type": "done"}
             ]},
            {"expect": {"protocol": "anthropic", "toolResultId": "ap1"},
             "chunks": [
                {"type": "text", "text": "patch round-trip complete"},
                {"type": "done"}
             ]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    std::fs::write(h.cwd().join("existing.txt"), "old value\n").unwrap();
    std::fs::write(h.cwd().join("obsolete.txt"), "remove me\n").unwrap();

    let out = h.run_turn("please apply the envelope patch").await;
    assert!(
        out.error.is_none() && out.finished && out.text.contains("patch round-trip complete"),
        "turn failed: {out:?}"
    );
    assert_eq!(out.tool_result_count(), 1, "events: {:?}", out.events);
    assert!(
        out.events.iter().any(|event| {
            event.starts_with("ToolCall") && event.contains("ApplyPatch") && event.contains("ap1")
        }),
        "the HTTP ToolUse must reach the production registry; events: {:?}",
        out.events
    );
    assert!(
        out.events.iter().any(|event| {
            event.starts_with("ToolResult")
                && event.contains("ap1")
                && event.contains("is_error: false")
        }),
        "ApplyPatch must produce a successful ToolResult; events: {:?}",
        out.events
    );

    assert_eq!(
        std::fs::read_to_string(h.cwd().join("existing.txt")).unwrap(),
        "updated value\n"
    );
    assert!(!h.cwd().join("obsolete.txt").exists());
    assert_eq!(
        std::fs::read_to_string(h.cwd().join("alpha.txt")).unwrap(),
        "alpha\n"
    );
    assert_eq!(
        std::fs::read_to_string(h.cwd().join("nested/beta.txt")).unwrap(),
        "beta\n"
    );
    assert_eq!(
        std::fs::read_to_string(h.cwd().join("file with spaces.txt")).unwrap(),
        "spaces\n"
    );

    let diff = out
        .events
        .iter()
        .find(|event| event.starts_with("TurnDiffSummary"))
        .expect("a successful ApplyPatch must emit TurnDiffSummary");
    for path in [
        "existing.txt",
        "obsolete.txt",
        "alpha.txt",
        "nested/beta.txt",
        "file with spaces.txt",
    ] {
        assert!(
            diff.contains(path),
            "TurnDiffSummary must contain {path:?}; event: {diff}"
        );
    }

    let journal = h.journal().await;
    assert_eq!(
        journal.as_array().map(Vec::len),
        Some(2),
        "journal: {journal}"
    );
    assert_eq!(journal[0]["protocol"], "anthropic", "journal: {journal}");
    assert_eq!(journal[0]["matched"], true, "journal: {journal}");
    assert!(
        journal[0]["toolNames"]
            .as_array()
            .is_some_and(|names| names.iter().any(|name| name == "ApplyPatch")),
        "the production provider request must advertise ApplyPatch; journal: {journal}"
    );
    assert_eq!(
        journal[1]["toolResultIds"],
        json!(["ap1"]),
        "journal: {journal}"
    );
    assert_eq!(
        journal[1]["toolResultErrorIds"],
        json!([]),
        "the successful ToolResult must not be encoded as an error; journal: {journal}"
    );
    assert_eq!(journal[1]["matched"], true, "journal: {journal}");

    let verify = h.verify().await;
    assert_eq!(verify["name"], "apply_patch_envelope", "verify: {verify}");
    assert_eq!(verify["served"], 2, "verify: {verify}");
    assert_eq!(verify["remaining"], 0, "verify: {verify}");
    assert_eq!(verify["requestCount"], 2, "verify: {verify}");
    assert_eq!(verify["unmatchedRequests"], 0, "verify: {verify}");
    assert_eq!(verify["verified"], true, "verify: {verify}");
}

/// A best-effort patch may mutate earlier files before a later operation
/// fails. The real child must retain those side effects in typed metadata,
/// publish an exact diff summary, and encode the ToolResult as an error on the
/// next Anthropic request.
#[tokio::test]
async fn partial_apply_patch_preserves_committed_paths_over_http() {
    const PATCH: &str = "\
*** Begin Patch
*** Add File: committed-a.txt
+first
*** Add File: committed-b.txt
+second
*** Add File: blocker/never.txt
+third
*** End Patch
";

    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "apply_patch_partial_commit",
        "calls": [
            {"expect": {"protocol": "anthropic", "userContains": "apply the partial patch"},
             "chunks": [
                 {"type": "tool_use", "id": "ap-partial", "name": "ApplyPatch",
                  "input": {"patch": PATCH}},
                 {"type": "done"}
             ]},
            {"expect": {"protocol": "anthropic", "toolResultId": "ap-partial",
                        "bodyContains": "already applied"},
             "chunks": [
                 {"type": "text", "text": "partial patch failure handled"},
                 {"type": "done"}
             ]}
        ]
    }))
    .await;
    std::fs::write(h.cwd().join("blocker"), "not a directory\n").unwrap();

    let out = h.run_turn("please apply the partial patch").await;
    assert!(
        out.error.is_none() && out.finished && out.text.contains("partial patch failure handled"),
        "turn failed: {out:?}"
    );
    assert_eq!(
        std::fs::read_to_string(h.cwd().join("committed-a.txt")).unwrap(),
        "first\n"
    );
    assert_eq!(
        std::fs::read_to_string(h.cwd().join("committed-b.txt")).unwrap(),
        "second\n"
    );
    assert!(!h.cwd().join("blocker/never.txt").exists());

    let tool_result = out
        .events
        .iter()
        .find(|event| event.starts_with("ToolResult") && event.contains("ap-partial"))
        .expect("partial ApplyPatch must publish its typed ToolResult");
    assert!(
        tool_result.contains("is_error: true"),
        "event: {tool_result}"
    );
    assert!(
        tool_result.contains("ModifiedFiles")
            && tool_result.contains("committed-a.txt")
            && tool_result.contains("committed-b.txt"),
        "partial side-effect metadata was lost: {tool_result}"
    );

    let diff = out
        .events
        .iter()
        .find(|event| event.starts_with("TurnDiffSummary"))
        .expect("partial ApplyPatch must emit TurnDiffSummary");
    assert!(
        diff.contains("committed-a.txt") && diff.contains("committed-b.txt"),
        "committed paths missing from diff: {diff}"
    );
    assert!(
        !diff.contains("blocker/never.txt"),
        "failed operation must not enter the diff: {diff}"
    );

    let journal = h.journal().await;
    assert_eq!(journal[1]["toolResultIds"], json!(["ap-partial"]));
    assert_eq!(journal[1]["toolResultErrorIds"], json!(["ap-partial"]));
    assert_eq!(journal[1]["matched"], true, "journal: {journal}");

    let verify = h.verify().await;
    assert_eq!(verify["served"], 2, "verify: {verify}");
    assert_eq!(verify["remaining"], 0, "verify: {verify}");
    assert_eq!(verify["verified"], true, "verify: {verify}");
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
/// conversation. Two lines stay below the capture layer's per-line limit while
/// their aggregate exceeds the pipeline overflow limit.
#[tokio::test]
async fn huge_tool_output_overflows_to_file() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "fs_overflow",
        "calls": [
            {"expect": {"userContains": "dump the blob"},
             "chunks": [
                {"type": "tool_use", "id": "o1", "name": "Bash",
                 "input": {"command": "for _ in 1 2; do head -c 60000 /dev/zero | tr '\\0' x; printf '\\n'; done"}},
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
        "a 120KB two-line output must trigger the overflow-to-file path; \
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
