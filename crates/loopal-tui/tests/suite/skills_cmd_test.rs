/// `/skills` command tests: opens sub-page, displays correct state.
use std::path::PathBuf;

use loopal_protocol::{ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::{App, SubPage};

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

fn write_skill(dir: &std::path::Path, filename: &str, content: &str) {
    let skills_dir = dir.join(".loopal").join("skills");
    std::fs::create_dir_all(&skills_dir).unwrap();
    std::fs::write(skills_dir.join(filename), content).unwrap();
}

fn extract_skills_page(app: &App) -> &loopal_tui::app::SkillsPageState {
    match app.sub_page.as_ref().expect("sub_page should be set") {
        SubPage::SkillsPage(s) => s,
        _ => panic!("expected SkillsPage variant"),
    }
}

// ---------------------------------------------------------------------------
// No skills — sub-page opens with empty list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_skills_cmd_no_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = make_app_with_cwd(tmp.path().to_path_buf());
    let handler = app.command_registry.find("/skills").unwrap();
    let effect = handler.execute(&mut app, None).await;
    assert!(matches!(effect, loopal_tui::command::CommandEffect::Done));
    let state = extract_skills_page(&app);
    assert!(state.skills.is_empty());
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

    let state = extract_skills_page(&app);
    assert_eq!(state.skills.len(), 1);
    assert_eq!(state.skills[0].name, "/commit");
    assert_eq!(state.skills[0].source, "project");
    assert_eq!(state.skills[0].description, "Generate git commit");
}

// ---------------------------------------------------------------------------
// Multiple skills — sorted alphabetically
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

    let state = extract_skills_page(&app);
    assert_eq!(state.skills.len(), 2);
    assert_eq!(state.skills[0].name, "/audit");
    assert_eq!(state.skills[1].name, "/deploy");
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

// ---------------------------------------------------------------------------
// Selected defaults to 0
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_skills_page_initial_selection() {
    let tmp = tempfile::tempdir().unwrap();
    write_skill(
        tmp.path(),
        "test.md",
        "---\ndescription: Test\n---\nTest.\n",
    );
    let mut app = make_app_with_cwd(tmp.path().to_path_buf());
    let handler = app.command_registry.find("/skills").unwrap();
    handler.execute(&mut app, None).await;

    let state = extract_skills_page(&app);
    assert_eq!(state.selected, 0);
    assert_eq!(state.scroll_offset, 0);
}
