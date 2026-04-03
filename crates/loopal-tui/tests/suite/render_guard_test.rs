/// Tests for render guard: zero-height agents area must not panic.
use loopal_protocol::{BgTaskSnapshot, BgTaskStatus, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

fn make_app() -> App {
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
    App::new(session, std::env::temp_dir())
}

/// Rendering at height=1 should not panic even with bg tasks present.
#[test]
fn render_tiny_terminal_with_bg_tasks_no_panic() {
    let mut app = make_app();
    app.bg_snapshots = vec![
        BgTaskSnapshot {
            id: "bg_1".into(),
            description: "task".into(),
            status: BgTaskStatus::Running,
        },
    ];
    let backend = TestBackend::new(80, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    // Should not panic — zero-height guard prevents invalid split.
    terminal.draw(|f| loopal_tui::render::draw(f, &mut app)).unwrap();
}

/// Rendering at height=3 with both agents area and bg tasks should not panic.
#[test]
fn render_small_terminal_no_panic() {
    let mut app = make_app();
    app.bg_snapshots = vec![
        BgTaskSnapshot {
            id: "bg_1".into(),
            description: "build".into(),
            status: BgTaskStatus::Running,
        },
    ];
    let backend = TestBackend::new(80, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| loopal_tui::render::draw(f, &mut app)).unwrap();
}
