//! Memory consolidation trigger — spawns a sub-agent for periodic consolidation.

use std::sync::Arc;

use tracing::{info, warn};

use loopal_agent::shared::AgentShared;
use loopal_agent::spawn::{SpawnParams, SpawnTarget, spawn_agent, wait_agent};
use loopal_memory::MEMORY_CONSOLIDATION_PROMPT;

use super::memory_adapter::ServerMemoryProcessor;

/// Trigger a full memory consolidation via a dedicated sub-agent.
///
/// Runs in the background (non-blocking). Uses a `.consolidation_lock` file
/// as an optimistic lock to prevent concurrent consolidations.
pub fn trigger_consolidation(shared: &Arc<AgentShared>, model: &str) {
    let memory_dir = shared.cwd.join(".loopal/memory");

    let lock_path = match loopal_memory::consolidation::try_acquire_lock(&memory_dir) {
        Some(path) => path,
        None => {
            info!("consolidation already in progress, skipping");
            return;
        }
    };

    let shared = shared.clone();
    let model = model.to_string();
    tokio::spawn(async move {
        let memory_dir = shared.cwd.join(".loopal/memory");
        let today = loopal_memory::date::today_str();
        let name = ServerMemoryProcessor::make_agent_name("memory-consolidation");
        let prompt = format!("{MEMORY_CONSOLIDATION_PROMPT}\n\nToday: {today}");
        let params = SpawnParams {
            name: name.clone(),
            prompt,
            model: Some(model),
            permission_mode: None,
            agent_type: None,
            depth: shared.depth + 1,
            target: SpawnTarget::InHub {
                cwd_override: None,
                fork_context: None,
            },
        };
        match spawn_agent(&shared, params).await {
            Ok(_) => {
                info!("memory consolidation agent spawned");
                match wait_agent(&shared, &name).await {
                    Ok(output) => {
                        info!(output = %output, "memory consolidation done");
                        loopal_memory::consolidation::mark_done(&memory_dir);
                    }
                    Err(e) => warn!(error = %e, "memory consolidation failed"),
                }
            }
            Err(e) => warn!(error = %e, "failed to spawn consolidation agent"),
        }
        loopal_memory::consolidation::release_lock(&lock_path);
    });
}
