pub mod app;
pub mod command;
pub mod event;
pub mod input;
mod key_dispatch;
mod key_dispatch_apply;
mod key_dispatch_ops;
mod key_dispatch_subpage;
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
    pub use crate::key_dispatch_ops::{cycle_panel_focus, enter_panel, handle_effect, panel_tab};
    pub use crate::question_ops::{compute_question_answers, route_paste};
}

/// Async dispatch table for e2e tests that drive `App` via real `InputAction`s.
/// Distinct from `dispatch_ops` (pure helpers) — this module owns side effects.
///
/// Single source of truth: forwards to
/// [`apply_action`](crate::key_dispatch_apply::apply_action), the same
/// function the production event loop uses. `DispatchOutcome::Quit` and
/// `PasteRequested` are intentionally discarded — tests assert on
/// observable side effects, not on the loop control signal.
#[doc(hidden)]
pub mod key_dispatch_for_test {
    use crate::app::App;
    use crate::input::InputAction;
    use crate::key_dispatch_apply::apply_action;

    pub async fn dispatch(app: &mut App, action: InputAction) {
        let _ = apply_action(app, action).await;
    }
}
