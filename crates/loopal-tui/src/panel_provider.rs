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
    fn max_visible(&self) -> usize;
    fn item_ids(&self, app: &App) -> Vec<String>;
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
