//! Panel zone focus navigation — driven by PanelRegistry.
//!
//! All per-kind dispatch goes through the registry. Adding a new panel
//! requires zero changes here — only a new PanelProvider impl + register call.

use crate::app::{App, FocusMode, PanelKind};
use crate::panel_state::PanelSectionState;

/// Enter Panel focus mode. Picks the first non-empty section.
pub fn enter_panel(app: &mut App) {
    let kinds = non_empty_kinds(app);
    let Some(&first) = kinds.first() else { return };
    app.focus_mode = FocusMode::Panel(first);
    ensure_focus(app, first);
}

/// Tab within the panel zone: cycle through non-empty sections.
pub fn panel_tab(app: &mut App) {
    let current = match app.focus_mode {
        FocusMode::Panel(k) => k,
        _ => return,
    };
    let kinds = non_empty_kinds(app);
    if kinds.len() <= 1 {
        cycle_panel_focus(app, true);
        return;
    }
    if let Some(pos) = kinds.iter().position(|&k| k == current) {
        let next = kinds[(pos + 1) % kinds.len()];
        app.focus_mode = FocusMode::Panel(next);
        ensure_focus(app, next);
    }
}

/// Navigate up/down within the currently active panel.
pub fn cycle_panel_focus(app: &mut App, forward: bool) {
    let kind = match app.focus_mode {
        FocusMode::Panel(k) => k,
        _ => return,
    };
    let Some(provider) = app.panel_registry.by_kind(kind) else {
        return;
    };
    let ids = provider.item_ids(app);
    let max = provider.max_visible();
    if ids.is_empty() {
        let section = app.section_mut(kind);
        section.focused = None;
        section.scroll_offset = 0;
        fallback(app);
        return;
    }
    let section = app.section_mut(kind);
    section.focused = Some(next_in_list(&ids, section.focused.as_deref(), forward));
    adjust_scroll(section, &ids, max);
}

pub(crate) fn has_live_agents(app: &App) -> bool {
    app.panel_registry
        .by_kind(PanelKind::Agents)
        .is_some_and(|p| !p.item_ids(app).is_empty())
}

// --- Generic helpers ---

fn ensure_focus(app: &mut App, kind: PanelKind) {
    let Some(provider) = app.panel_registry.by_kind(kind) else {
        return;
    };
    let ids = provider.item_ids(app);
    let max = provider.max_visible();
    let section = app.section_mut(kind);
    if section.focused.as_ref().is_none_or(|f| !ids.contains(f)) {
        section.focused = ids.first().cloned();
        adjust_scroll(section, &ids, max);
    }
}

fn fallback(app: &mut App) {
    let kinds = non_empty_kinds(app);
    if let Some(&first) = kinds.first() {
        app.focus_mode = FocusMode::Panel(first);
        ensure_focus(app, first);
    } else {
        app.focus_mode = FocusMode::Input;
    }
}

fn non_empty_kinds(app: &App) -> Vec<PanelKind> {
    app.panel_registry
        .providers()
        .iter()
        .filter(|p| !p.item_ids(app).is_empty())
        .map(|p| p.kind())
        .collect()
}

fn next_in_list(items: &[String], current: Option<&str>, forward: bool) -> String {
    let pos = current.and_then(|c| items.iter().position(|k| k == c));
    let idx = match pos {
        Some(i) if forward => (i + 1) % items.len(),
        Some(i) => (i + items.len() - 1) % items.len(),
        None if forward => 0,
        None => items.len() - 1,
    };
    items[idx].clone()
}

fn adjust_scroll(section: &mut PanelSectionState, ids: &[String], max_visible: usize) {
    let total = ids.len();
    if max_visible == 0 || total <= max_visible {
        section.scroll_offset = 0;
        return;
    }
    let idx = section
        .focused
        .as_ref()
        .and_then(|f| ids.iter().position(|k| k == f))
        .unwrap_or(0);
    if idx < section.scroll_offset {
        section.scroll_offset = idx;
    } else if idx >= section.scroll_offset + max_visible {
        section.scroll_offset = idx + 1 - max_visible;
    }
    section.scroll_offset = section.scroll_offset.min(total.saturating_sub(max_visible));
}
