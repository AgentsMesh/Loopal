pub mod app;
pub mod command;
pub mod event;
pub mod input;
mod key_dispatch;
mod key_dispatch_ops;
pub mod markdown;
pub mod render;
mod render_layout;
pub mod terminal;
pub(crate) mod text_util;
mod tui_loop;
pub mod views;

pub use tui_loop::{run_tui, run_tui_loop};
