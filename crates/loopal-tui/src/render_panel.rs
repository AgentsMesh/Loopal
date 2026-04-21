//! Panel zone rendering — section headers + active-only focus highlight.
//!
//! Renders the stack of registered panel providers. When ≥2 panels have
//! content, a single-line section header (`━━ Title (count) ━━…`) is
//! drawn above each panel, acting as both label and separator. Only the
//! currently active panel (per `FocusMode::Panel(kind)`) receives its
//! `section.focused` id — other panels are rendered with `None`, which
//! hides their ` ▸ ` indicator. The underlying state is preserved so
//! Tab-ing back restores the prior selection.

use std::time::Duration;

use ratatui::prelude::*;

use crate::app::{App, FocusMode, PanelKind};
use crate::panel_provider::PanelProvider;
use crate::views::panel_header;

/// Minimum number of visible panels required to show section headers.
/// A lone panel is self-evident and doesn't need a label.
const HEADER_MIN_PANELS: usize = 2;

/// Total vertical height the panel zone needs for this frame.
///
/// Accounts for the optional per-panel header row when ≥2 panels are
/// visible. Mirrors the layout done in `render_panel_zone` to ensure
/// `FrameLayout` allocates the right amount of space.
pub fn panel_zone_height(app: &App, state: &loopal_session::state::SessionState) -> u16 {
    let visible = visible_providers(app, state);
    let content: u16 = visible.iter().map(|(_, h)| *h).sum();
    let headers = if show_headers(visible.len()) {
        visible.len() as u16
    } else {
        0
    };
    content + headers
}

/// Render the panel zone into `area`.
pub fn render_panel_zone(
    f: &mut Frame,
    app: &App,
    state: &loopal_session::state::SessionState,
    elapsed: Duration,
    area: Rect,
) {
    if area.height == 0 {
        return;
    }
    let visible = visible_providers(app, state);
    if visible.is_empty() {
        return;
    }
    let with_header = show_headers(visible.len());
    let header_h: u16 = u16::from(with_header);
    let active_kind = match app.focus_mode {
        FocusMode::Panel(k) => Some(k),
        _ => None,
    };

    let constraints: Vec<Constraint> = visible
        .iter()
        .map(|(_, h)| Constraint::Length(*h + header_h))
        .collect();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for ((provider, content_h), &chunk) in visible.iter().zip(chunks.iter()) {
        let content_area = if with_header {
            draw_header(f, app, state, *provider, active_kind, chunk);
            Rect {
                x: chunk.x,
                y: chunk.y + 1,
                width: chunk.width,
                height: *content_h,
            }
        } else {
            chunk
        };
        let focused = active_focused_id(app, provider.kind(), active_kind);
        provider.render(f, app, state, focused, elapsed, content_area);
    }
}

/// Whether section headers should be drawn for `visible_len` panels.
///
/// Centralized so `panel_zone_height` and `render_panel_zone` cannot drift.
fn show_headers(visible_len: usize) -> bool {
    visible_len >= HEADER_MIN_PANELS
}

/// Collect `(provider, content_height)` for panels with non-zero height,
/// preserving registry order.
fn visible_providers<'a>(
    app: &'a App,
    state: &loopal_session::state::SessionState,
) -> Vec<(&'a dyn PanelProvider, u16)> {
    app.panel_registry
        .providers()
        .iter()
        .map(|p| (p.as_ref(), p.height(app, state)))
        .filter(|(_, h)| *h > 0)
        .collect()
}

/// Render the 1-row section header above `chunk`.
fn draw_header(
    f: &mut Frame,
    app: &App,
    state: &loopal_session::state::SessionState,
    provider: &dyn PanelProvider,
    active_kind: Option<PanelKind>,
    chunk: Rect,
) {
    let header_area = Rect {
        x: chunk.x,
        y: chunk.y,
        width: chunk.width,
        height: 1,
    };
    let is_active = active_kind == Some(provider.kind());
    let count = provider.count(app, state);
    panel_header::render_section_header(f, provider.title(), count, is_active, header_area);
}

/// Return the focused id only for the panel that owns input focus.
/// Inactive panels get `None` so their ` ▸ ` indicator is hidden even
/// though `section.focused` may still hold a remembered selection.
fn active_focused_id(app: &App, kind: PanelKind, active_kind: Option<PanelKind>) -> Option<&str> {
    if active_kind == Some(kind) {
        app.section(kind).focused.as_deref()
    } else {
        None
    }
}
