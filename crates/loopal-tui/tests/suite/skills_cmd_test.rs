/// `/skills` command tests: output formatting, source display, edge cases.
use std::path::PathBuf;

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;

use tokio::sync::mpsc;

fn make_app_with_cwd(cwd: PathBuf) -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (perm_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        "test-model".into(),
        "act".into(),
        control_tx,
        perm_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, cwd)
}

fn last_system_message(app: &App) -> String {
    let state = app.session.lock();
    state
        .active_conversation()
        .messages
        .last()
        .expect("expected a system message")
        .content
        .clone()
}

fn write_skill(dir: &std::path::Path, filename: &str, content: &str) {
    let skills_dir = dir.join(".loopal").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();
    std::fs::write(skills_dir.join(filename), content).unwrap();
}

// ---------------------------------------------------------------------------
// No skills
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_skills_cmd_no_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app_with_cwd(tmp.path().to_path_buf());
    let handler = app.command_registry.find("/skills").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(effect, loopal_tui::command::CommandEffect::Done));
    assert_eq!(last_system_message(&app), "No skills loaded.");
}

// ---------------------------------------------------------------------------
// Single skill from project layer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_skills_cmd_single_skill() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(
        tmp.path(),
        "commit.md",
        "---\ndescription: Generate git commit\n---\nReview changes.\n",
    );
    let mut app = make_app_with_cwd(tmp.path().to_path_buf());
    let handler = app.command_registry.find("/skills").unwrap();
    handler.execute(&mut app, None).await;

    let msg = last_system_message(&app);
    assert!(msg.contains("Loaded skills (1):"));
    assert!(msg.contains("/commit"));
    assert!(msg.contains("[project]"));
    assert!(msg.contains("Generate git commit"));
}

// ---------------------------------------------------------------------------
// Multiple skills — sorted, all listed
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_skills_cmd_multiple_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(
        tmp.path(),
        "deploy.md",
        "---\ndescription: Deploy app\n---\nDeploy.\n",
    );
    write_skill(
        tmp.path(),
        "audit.md",
        "---\ndescription: Run audit\n---\nAudit.\n",
    );
    let mut app = make_app_with_cwd(tmp.path().to_path_buf());
    let handler = app.command_registry.find("/skills").unwrap();
    handler.execute(&mut app, None).await;

    let msg = last_system_message(&app);
    assert!(msg.contains("Loaded skills (2):"));
    let audit_pos = msg.find("/audit").expect("missing /audit");
    let deploy_pos = msg.find("/deploy").expect("missing /deploy");
    assert!(
        audit_pos < deploy_pos,
        "skills should be sorted alphabetically"
    );
}

// ---------------------------------------------------------------------------
// Source legend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_skills_cmd_source_legend() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(
        tmp.path(),
        "test.md",
        "---\ndescription: Test\n---\nTest.\n",
    );
    let mut app = make_app_with_cwd(tmp.path().to_path_buf());
    let handler = app.command_registry.find("/skills").unwrap();
    handler.execute(&mut app, None).await;

    let msg = last_system_message(&app);
    assert!(msg.contains("Sources: project"));
}

// ---------------------------------------------------------------------------
// /skills is builtin, not a skill
// ---------------------------------------------------------------------------

#[test]
fn test_skills_cmd_is_builtin() {
    let tmp = tempfile::tempdir().unwrap();
    let app = make_app_with_cwd(tmp.path().to_path_buf());
    let handler = app.command_registry.find("/skills").unwrap();
    assert!(!handler.is_skill());
    assert!(handler.skill_body().is_none());
}
