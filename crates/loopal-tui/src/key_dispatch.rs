//! Key-action dispatch — translates `DispatchOutcome` into the quit flag
//! consumed by the TUI event loop. All side effects live in
//! [`apply_action`](crate::key_dispatch_apply::apply_action) so this file
//! and the test dispatch table share one source of truth.

use crate::app::App;
use crate::event::EventHandler;
use crate::input::{handle_key, paste};
use crate::key_dispatch_apply::{DispatchOutcome, apply_action};

/// Process a single key event and return `true` if the TUI should quit.
pub(crate) async fn handle_key_action(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    events: &EventHandler,
) -> bool {
    let action = handle_key(app, key);
    match apply_action(app, action).await {
        DispatchOutcome::Continue => false,
        DispatchOutcome::Quit => {
            app.exiting = true;
            true
        }
        DispatchOutcome::PasteRequested => {
            paste::spawn_paste(events);
            false
        }
    }
}
