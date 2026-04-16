//! Panel provider implementations and registration.

mod agent_provider;
mod bg_tasks_provider;
mod tasks_provider;

use crate::panel_provider::PanelRegistry;

pub fn register_all(registry: &mut PanelRegistry) {
    registry.register(Box::new(agent_provider::AgentPanelProvider));
    registry.register(Box::new(tasks_provider::TasksPanelProvider));
    registry.register(Box::new(bg_tasks_provider::BgTasksPanelProvider));
}
