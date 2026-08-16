pub mod collaboration;
pub mod cron;
pub mod task;
pub mod workflow;

use std::sync::Arc;

use loopal_kernel::Kernel;
use loopal_memory::{MemoryGraph, MemoryImportanceTool, MemoryRecallTool};

/// Register all agent tools into the kernel.
pub fn register_all(kernel: &mut Kernel) {
    // Collaboration tools (Hub-based multi-agent)
    kernel.register_tool(Box::new(collaboration::agent::AgentTool));
    kernel.register_tool(Box::new(collaboration::send_message::SendMessageTool));
    kernel.register_tool(Box::new(collaboration::list_hubs::ListHubsTool));
    // Agent-internal tools
    kernel.register_tool(Box::new(task::TaskCreateTool));
    kernel.register_tool(Box::new(task::TaskUpdateTool));
    kernel.register_tool(Box::new(task::TaskListTool));
    kernel.register_tool(Box::new(task::TaskGetTool));
    kernel.register_tool(Box::new(loopal_memory::MemoryTool));
    // Cron scheduling tools
    kernel.register_tool(Box::new(cron::CronCreateTool));
    kernel.register_tool(Box::new(cron::CronDeleteTool));
    kernel.register_tool(Box::new(cron::CronListTool));
}

pub fn register_workflow(kernel: &Kernel) {
    workflow::register(kernel);
}

/// Register the `memory_recall` tool. Called separately because it requires
/// the MemoryGraph instance, which is initialized by the agent server after
/// scanning the project's `.loopal/memory/` directory.
pub fn register_memory_recall(kernel: &mut Kernel, graph: Arc<MemoryGraph>) {
    kernel.register_tool(Box::new(MemoryRecallTool::new(graph.clone())));
    kernel.register_tool(Box::new(MemoryImportanceTool::new(graph)));
}
