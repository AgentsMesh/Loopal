use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use loopal_protocol::{AgentEvent, AgentEventPayload, ControlCommand, UserQuestionResponse};
use loopal_session::SessionController;
use loopal_tui::app::App;
use loopal_tui::input::{InputAction, handle_key};
use loopal_tui::key_dispatch_for_test;
use loopal_tui::views::plan_approval_inline;
use loopal_view_state::PendingPlanApproval;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use tokio::sync::mpsc;

fn make_app() -> App {
    let (control_tx, _) = mpsc::channel::<ControlCommand>(16);
    let (permission_tx, _) = mpsc::channel::<bool>(16);
    let (question_tx, _) = mpsc::channel::<UserQuestionResponse>(16);
    let session = SessionController::new(
        control_tx,
        permission_tx,
        question_tx,
        Default::default(),
        std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
    );
    App::new(session, std::env::temp_dir())
}

fn request_plan(app: &mut App, content: &str) {
    app.dispatch_event(AgentEvent::root(AgentEventPayload::PlanApprovalRequest {
        id: "plan-1".into(),
        plan_content: content.into(),
        plan_path: "/tmp/plans/feature.md".into(),
    }));
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[path = "plan_approval_test/interaction_test.rs"]
mod interaction_test;
#[path = "plan_approval_test/render_test.rs"]
mod render_test;
