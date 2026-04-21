//! PanelProvider trait + registry — complete panel definition in one place.
//!
//! Each provider defines kind, data queries, and rendering for one panel.
//! The registry holds all providers; panel_ops and render iterate over it.
//! Adding a new panel = one PanelProvider impl + one register call.

use std::time::Duration;

use loopal_session::state::SessionState;
use ratatui::prelude::*;

use crate::app::{App, PanelKind};

pub trait PanelProvider: Send + Sync {
    fn kind(&self) -> PanelKind;
    /// Short label shown in the section header when multiple panels are visible.
    fn title(&self) -> &'static str;
    fn max_visible(&self) -> usize;
    /// List of item identifiers in the panel, read from the passed-in
    /// `state`.
    ///
    /// Taking `state` as a parameter (rather than calling
    /// `app.session.lock()` internally) avoids lock-reentrancy deadlocks
    /// when render-time callers already hold the session guard.
    fn item_ids(&self, app: &App, state: &SessionState) -> Vec<String>;
    /// Number of items in the panel.
    ///
    /// Default delegates to `item_ids(...).len()`. Providers should
    /// override with an allocation-free count (`iter().filter().count()`)
    /// when `item_ids` would otherwise build a throwaway `Vec<String>`
    /// — the section header only needs the integer.
    fn count(&self, app: &App, state: &SessionState) -> usize {
        self.item_ids(app, state).len()
    }
    fn height(&self, app: &App, state: &SessionState) -> u16;
    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        state: &SessionState,
        focused: Option<&str>,
        elapsed: Duration,
        area: Rect,
    );
}

pub struct PanelRegistry {
    providers: Vec<Box<dyn PanelProvider>>,
}

impl Default for PanelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PanelRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn PanelProvider>) {
        self.providers.push(provider);
    }

    pub fn providers(&self) -> &[Box<dyn PanelProvider>] {
        &self.providers
    }

    pub fn by_kind(&self, kind: PanelKind) -> Option<&dyn PanelProvider> {
        self.providers
            .iter()
            .find(|p| p.kind() == kind)
            .map(|p| p.as_ref())
    }
}
