pub mod app;
pub mod command;
pub mod event;
pub mod input;
mod key_dispatch;
pub mod markdown;
pub mod render;
pub mod terminal;
mod tui_helpers;
mod tui_loop;
pub mod views;

pub use tui_loop::{run_tui, run_tui_loop};
