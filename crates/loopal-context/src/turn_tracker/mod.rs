mod derive;
mod ephemeral;
mod error;
mod logger;
mod persisted;
mod tool_batch;
pub mod wire_mutator;

pub use error::TurnTrackerError;
pub use logger::{PersistError, TurnEventLogger};

use loopal_turn::TurnId;

use self::derive::derive_current_tool_batch_step;
use crate::budget::ContextBudget;
use crate::store::ProjectedView;
use crate::turn_store::TurnStore;

pub struct TurnTracker {
    pub(super) store: TurnStore,
    pub(super) view: ProjectedView,
    pub(super) current_tool_batch_step: Option<u32>,
}

impl TurnTracker {
    pub fn new(store: TurnStore, budget: ContextBudget) -> Self {
        let mut view = ProjectedView::new(budget);
        view.refresh_view(store.turns());
        let current_tool_batch_step = derive_current_tool_batch_step(&store);
        Self {
            store,
            view,
            current_tool_batch_step,
        }
    }

    pub fn current_turn_id(&self) -> Option<&TurnId> {
        self.store.current_turn_id()
    }
    pub fn current_tool_batch_step(&self) -> Option<u32> {
        self.current_tool_batch_step
    }
    pub fn store(&self) -> &TurnStore {
        &self.store
    }
    pub fn view(&self) -> &ProjectedView {
        &self.view
    }

    // Re-projects the view so new ingestion caps apply immediately. Without
    // the refresh, cached messages retain caps from the old budget until
    // the next turn mutator triggers refresh_view.
    pub fn update_budget(&mut self, budget: ContextBudget) {
        self.view.update_budget(budget);
        self.refresh_view();
    }

    pub fn record_actual_input_tokens(&mut self, tokens: u32) {
        self.view.record_actual_input_tokens(tokens);
    }

    pub fn replace_store(&mut self, store: TurnStore) {
        self.store = store;
        self.view.refresh_view(self.store.turns());
        self.current_tool_batch_step = derive_current_tool_batch_step(&self.store);
    }

    pub(super) fn refresh_view(&mut self) {
        self.view.refresh_view(self.store.turns());
    }
}
