pub mod app;
pub mod command;
pub mod event;
pub mod input;
mod key_dispatch;
mod key_dispatch_ops;
pub mod markdown;
mod panel_ops;
pub mod panel_provider;
pub mod panel_state;
pub mod providers;
mod question_ops;
pub mod render;
mod render_layout;
pub mod render_panel;
mod resume_display;
mod session_cleanup;
pub mod terminal;
pub(crate) mod text_util;
mod tui_loop;
mod tui_sync;
pub mod view_client;
pub mod views;

pub use terminal::install_panic_hook;
pub use tui_loop::{ExitInfo, run_tui, run_tui_loop};

/// Pure helpers re-exported for unit tests (synchronous, side-effect-free).
#[doc(hidden)]
pub mod dispatch_ops {
    pub use crate::key_dispatch_ops::{
        cycle_panel_focus, enter_panel, handle_effect, panel_tab,
    };
    pub use crate::question_ops::{compute_question_answers, route_paste};
}

/// Async dispatch table for e2e tests that drive `App` via real `InputAction`s.
/// Distinct from `dispatch_ops` (pure helpers) — this module owns side effects.
#[doc(hidden)]
pub mod key_dispatch_for_test {
    use crate::app::App;
    use crate::input::InputAction;

    pub async fn dispatch(app: &mut App, action: InputAction) {
        match action {
            InputAction::None => {}
            InputAction::QuestionConfirm => crate::question_ops::confirm(app).await,
            InputAction::QuestionCancel => crate::question_ops::cancel(app).await,
            InputAction::QuestionUp => crate::question_ops::cursor_up(app),
            InputAction::QuestionDown => crate::question_ops::cursor_down(app),
            InputAction::QuestionToggle => crate::question_ops::toggle(app),
            InputAction::QuestionFreeTextChar(c) => crate::question_ops::free_text_char(app, c),
            InputAction::QuestionFreeTextBackspace => crate::question_ops::free_text_backspace(app),
            InputAction::QuestionFreeTextDelete => crate::question_ops::free_text_delete(app),
            InputAction::QuestionFreeTextCursorLeft => {
                crate::question_ops::free_text_cursor_left(app)
            }
            InputAction::QuestionFreeTextCursorRight => {
                crate::question_ops::free_text_cursor_right(app)
            }
            InputAction::QuestionFreeTextHome => crate::question_ops::free_text_home(app),
            InputAction::QuestionFreeTextEnd => crate::question_ops::free_text_end(app),
            other => {
                panic!("key_dispatch_for_test: unhandled action {other:?}; extend dispatch table")
            }
        }
    }
}
