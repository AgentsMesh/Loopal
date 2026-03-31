//! Auto Mode integration tests: serde and mixed permission level batches.

use super::auto_mode_helpers::*;
use super::make_cancel;
use loopal_tool_api::PermissionMode;

// ── Mixed permission levels in one batch ───────────────────────────

/// Batch with ReadOnly + Supervised + Dangerous: only Dangerous classified.
#[tokio::test]
async fn mixed_batch_only_classifies_dangerous() {
    // One classifier call for the single DangerTool in the batch.
    let (mut runner, mut event_rx) = make_auto_runner(vec![allow_chunks()]);

    let tmp = std::env::temp_dir().join(format!("loopal_auto_mix_{}.txt", std::process::id()));
    std::fs::write(&tmp, "test").unwrap();
    runner.tool_ctx.backend = loopal_backend::LocalBackend::new(
        std::env::temp_dir(),
        None,
        loopal_backend::ResourceLimits::default(),
    );

    let tool_uses = vec![
        (
            "tc-1".into(),
            "Read".into(),
            serde_json::json!({"file_path": tmp.to_str().unwrap()}),
        ),
        (
            "tc-2".into(),
            "Write".into(),
            serde_json::json!({"file_path": "/tmp/mix_out.txt", "content": "x"}),
        ),
        (
            "tc-3".into(),
            "DangerTool".into(),
            serde_json::json!({"command": "cargo test"}),
        ),
    ];

    runner
        .execute_tools(tool_uses, &make_cancel())
        .await
        .unwrap();

    // Only DangerTool triggers classification.
    let decisions = drain_auto_decisions(&mut event_rx);
    assert_eq!(decisions.len(), 1, "only Dangerous tool classified");
    assert_eq!(decisions[0].0, "DangerTool");
    assert_eq!(decisions[0].1, "allow");

    let _ = std::fs::remove_file(&tmp);
}

/// Empty tool batch is a no-op (no panic, no classification).
#[tokio::test]
async fn empty_batch_is_noop() {
    let (mut runner, mut event_rx) = make_auto_runner(vec![]);

    let result = runner.execute_tools(vec![], &make_cancel()).await.unwrap();

    assert!(result.is_none());
    assert!(drain_auto_decisions(&mut event_rx).is_empty());
}

// ── Serde round-trip ───────────────────────────────────────────────

/// PermissionMode::Auto round-trips through JSON correctly.
#[test]
fn permission_mode_auto_serde_roundtrip() {
    let json = serde_json::to_string(&PermissionMode::Auto).unwrap();
    assert_eq!(json, r#""auto""#);
    let back: PermissionMode = serde_json::from_str(&json).unwrap();
    assert_eq!(back, PermissionMode::Auto);
}

/// All three permission mode variants serialize to snake_case.
#[test]
fn permission_mode_all_variants_serde() {
    for (mode, expected) in [
        (PermissionMode::Bypass, r#""bypass""#),
        (PermissionMode::Supervised, r#""supervised""#),
        (PermissionMode::Auto, r#""auto""#),
    ] {
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, expected, "serialize {mode:?}");
        let back: PermissionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, mode, "deserialize {expected}");
    }
}
