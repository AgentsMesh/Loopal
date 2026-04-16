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
pub mod render;
mod render_layout;
mod session_cleanup;
pub mod terminal;
pub(crate) mod text_util;
mod tui_loop;
pub mod views;

pub use terminal::install_panic_hook;
pub use tui_loop::{run_tui, run_tui_loop};

/// Re-exports of dispatch functions for integration testing.
#[doc(hidden)]
pub mod dispatch_ops {
    pub use crate::key_dispatch_ops::{cycle_panel_focus, enter_panel, panel_tab};
}
