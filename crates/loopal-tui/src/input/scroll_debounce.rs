//! Arrow-key debounce: distinguishes mouse-wheel bursts from keyboard presses.
//!
//! xterm alternate scroll (`\x1b[?1007h`) translates mouse wheel into Up/Down
//! arrow keys. This module uses timing to tell them apart:
//!   - Rapid-fire (< 30 ms gap) → mouse wheel → content scroll
//!   - Isolated (> 30 ms) → keyboard → history navigation
//!
//! State: `Idle → Pending (30 ms) → Scrolling (150 ms idle → Idle)`
//! Second arrow within window → burst → Scrolling.
//! Other key or timer expiry → flush Pending as history.

use std::time::{Duration, Instant};

use crossterm::event::KeyCode;

use super::InputAction;
use super::multiline;
use super::navigation::{DEFAULT_WRAP_WIDTH, handle_down, handle_up};
use crate::app::App;

/// Window within which a second arrow event is considered a mouse-wheel burst.
const BURST_DETECT_MS: u64 = 30;

/// After this idle period the scroll burst ends and state returns to Idle.
const SCROLL_IDLE_MS: u64 = 150;

/// Scroll direction derived from arrow key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollDirection {
    Up,
    Down,
}

impl ScrollDirection {
    pub(crate) fn from_key(code: KeyCode) -> Option<Self> {
        match code {
            KeyCode::Up => Some(Self::Up),
            KeyCode::Down => Some(Self::Down),
            _ => None,
        }
    }
}

/// Arrow-key debounce state.
#[derive(Debug, Default)]
pub(crate) enum ArrowDebounce {
    /// No pending arrow event.
    #[default]
    Idle,
    /// First arrow received; waiting to see if a burst follows.
    Pending {
        direction: ScrollDirection,
        time: Instant,
    },
    /// Mouse-wheel burst confirmed; subsequent arrows scroll content.
    Scrolling { last_time: Instant },
}

/// Called by `handle_input_mode_key` when Up or Down is pressed.
///
/// Multiline cursor navigation bypasses debounce entirely (immediate).
/// Otherwise returns `StartArrowDebounce` when the first arrow is deferred,
/// or `None` when handled inline (scroll / burst continuation).
pub(super) fn handle_arrow_with_debounce(app: &mut App, direction: ScrollDirection) -> InputAction {
    // Multiline cursor navigation is always immediate — never debounced.
    // This keeps multiline editing responsive and avoids burst misfires
    // from fast keyboard repeat in multi-line input fields.
    if try_multiline_nav(app, direction) {
        app.arrow_debounce = ArrowDebounce::Idle;
        return InputAction::None;
    }

    match app.arrow_debounce {
        ArrowDebounce::Idle => {
            app.arrow_debounce = ArrowDebounce::Pending {
                direction,
                time: Instant::now(),
            };
            InputAction::StartArrowDebounce
        }
        ArrowDebounce::Pending {
            direction: old_dir,
            time,
        } => {
            if time.elapsed() < burst_detect_duration() {
                // Second event within burst window → mouse-wheel burst → scroll.
                app.arrow_debounce = ArrowDebounce::Scrolling {
                    last_time: Instant::now(),
                };
                apply_scroll(app, old_dir);
                apply_scroll(app, direction);
                InputAction::None
            } else {
                // Timer was delayed. Flush stale pending as history, then
                // start a new debounce for this event.
                process_as_history(app, old_dir);
                app.arrow_debounce = ArrowDebounce::Pending {
                    direction,
                    time: Instant::now(),
                };
                InputAction::StartArrowDebounce
            }
        }
        ArrowDebounce::Scrolling { last_time } => {
            if last_time.elapsed() > Duration::from_millis(SCROLL_IDLE_MS) {
                // Tick was dropped or delayed. Treat as fresh Idle state.
                app.arrow_debounce = ArrowDebounce::Pending {
                    direction,
                    time: Instant::now(),
                };
                InputAction::StartArrowDebounce
            } else {
                app.arrow_debounce = ArrowDebounce::Scrolling {
                    last_time: Instant::now(),
                };
                apply_scroll(app, direction);
                InputAction::None
            }
        }
    }
}

/// Discard pending debounce without processing as history.
///
/// Used by modal/global/autocomplete handlers that supersede the pending
/// arrow event. The stale 30 ms timer will see `Idle` and become a no-op.
pub(crate) fn discard_pending(app: &mut App) {
    app.arrow_debounce = ArrowDebounce::Idle;
}

/// Flush any pending arrow as a history navigation action.
///
/// Called when a non-arrow key arrives or when the debounce timer expires.
pub(crate) fn resolve_pending_arrow(app: &mut App) {
    match std::mem::replace(&mut app.arrow_debounce, ArrowDebounce::Idle) {
        ArrowDebounce::Pending { direction, .. } => {
            process_as_history(app, direction);
        }
        ArrowDebounce::Scrolling { .. } | ArrowDebounce::Idle => {}
    }
}

/// Expire stale Scrolling state (called from Tick handler).
pub(crate) fn tick_debounce(app: &mut App) {
    if let ArrowDebounce::Scrolling { last_time } = app.arrow_debounce
        && last_time.elapsed() > Duration::from_millis(SCROLL_IDLE_MS)
    {
        app.arrow_debounce = ArrowDebounce::Idle;
    }
}

/// Burst detection window.
pub(crate) fn burst_detect_duration() -> Duration {
    Duration::from_millis(BURST_DETECT_MS)
}

fn try_multiline_nav(app: &mut App, direction: ScrollDirection) -> bool {
    if !multiline::is_multiline(&app.input, DEFAULT_WRAP_WIDTH) {
        return false;
    }
    let new_cursor = match direction {
        ScrollDirection::Up => {
            multiline::cursor_up(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH)
        }
        ScrollDirection::Down => {
            multiline::cursor_down(&app.input, app.input_cursor, DEFAULT_WRAP_WIDTH)
        }
    };
    if let Some(pos) = new_cursor {
        app.input_cursor = pos;
        true
    } else {
        false
    }
}

fn process_as_history(app: &mut App, direction: ScrollDirection) {
    match direction {
        ScrollDirection::Up => {
            handle_up(app);
        }
        ScrollDirection::Down => {
            handle_down(app);
        }
    }
}

fn apply_scroll(app: &mut App, direction: ScrollDirection) {
    match direction {
        ScrollDirection::Up => {
            app.scroll_offset = app.scroll_offset.saturating_add(3);
        }
        ScrollDirection::Down => {
            app.scroll_offset = app.scroll_offset.saturating_sub(3);
        }
    }
}
