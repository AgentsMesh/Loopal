#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::{bind_token_channel, cleanup_channel, fetch_token};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_sid;
#[cfg(windows)]
pub use windows::{bind_token_channel, cleanup_channel, fetch_token};
