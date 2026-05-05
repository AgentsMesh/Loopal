/// Tests for the refactored `/init` command — agent-powered project analysis.
use std::fs;

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::command::CommandEffect;

use tokio::sync::mpsc;

fn make_app_in(cwd: std::path::PathBuf) -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, cwd)
}

// ---------------------------------------------------------------------------
// Command effect
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_init_returns_inbox_push() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app_in(dir.path().to_path_buf());
    let handler = app.command_registry.find("/init").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(
        matches!(effect, CommandEffect::InboxPush(_)),
        "expected InboxPush, got Done/ModeSwitch/Quit"
    );
}

#[tokio::test]
async fn test_init_prompt_contains_cwd() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app_in(dir.path().to_path_buf());
    let handler = app.command_registry.find("/init").unwrap();
    let effect = handler.execute(&mut app, None).await;
    match effect {
        CommandEffect::InboxPush(content) => {
            assert!(content.text.contains("LOOPAL.md"));
            assert!(
                content
                    .text
                    .contains(&dir.path().to_string_lossy().to_string())
            );
        }
        _ => panic!("expected InboxPush"),
    }
}

// ---------------------------------------------------------------------------
// Scaffolding
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_init_creates_scaffolding_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app_in(dir.path().to_path_buf());
    let handler = app.command_registry.find("/init").unwrap();
    handler.execute(&mut app, None).await;

    assert!(dir.path().join(".loopal").is_dir());
    assert!(dir.path().join(".loopal/memory").is_dir());
    assert!(dir.path().join(".loopal/memory/MEMORY.md").exists());
}

#[tokio::test]
async fn test_init_does_not_create_loopal_md_directly() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app_in(dir.path().to_path_buf());
    let handler = app.command_registry.find("/init").unwrap();
    handler.execute(&mut app, None).await;

    // LOOPAL.md should NOT be created by init — the agent will create it.
    assert!(!dir.path().join("LOOPAL.md").exists());
}

// ---------------------------------------------------------------------------
// Existing LOOPAL.md handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_init_includes_existing_content_in_prompt() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("LOOPAL.md"),
        "# Old instructions\nKeep this.",
    )
    .unwrap();

    let mut app = make_app_in(dir.path().to_path_buf());
    let handler = app.command_registry.find("/init").unwrap();
    let effect = handler.execute(&mut app, None).await;
    match effect {
        CommandEffect::InboxPush(content) => {
            assert!(content.text.contains("# Old instructions"));
            assert!(content.text.contains("Keep this."));
            assert!(content.text.contains("Existing LOOPAL.md"));
        }
        _ => panic!("expected InboxPush"),
    }
}

#[tokio::test]
async fn test_init_system_message_shows_generate() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app_in(dir.path().to_path_buf());
    let handler = app.command_registry.find("/init").unwrap();
    handler.execute(&mut app, None).await;

    let conv = app.snapshot_active_conversation();
    let last = conv.messages.last().expect("expected system message");
    assert!(last.content.contains("generate LOOPAL.md"));
}

#[tokio::test]
async fn test_init_system_message_shows_update_when_existing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("LOOPAL.md"), "existing").unwrap();

    let mut app = make_app_in(dir.path().to_path_buf());
    let handler = app.command_registry.find("/init").unwrap();
    handler.execute(&mut app, None).await;

    let conv = app.snapshot_active_conversation();
    let last = conv.messages.last().expect("expected system message");
    assert!(last.content.contains("update LOOPAL.md"));
}
