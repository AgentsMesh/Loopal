pub mod collaboration;
pub mod completion;
pub mod task;

use loopal_kernel::Kernel;

/// Register all agent tools into the kernel.
pub fn register_all(kernel: &mut Kernel) {
    // Collaboration tools (Hub-based multi-agent)
    kernel.register_tool(Box::new(collaboration::agent::AgentTool));
    kernel.register_tool(Box::new(collaboration::send_message::SendMessageTool));
    // Agent-internal tools
    kernel.register_tool(Box::new(task::TaskCreateTool));
    kernel.register_tool(Box::new(task::TaskUpdateTool));
    kernel.register_tool(Box::new(task::TaskListTool));
    kernel.register_tool(Box::new(task::TaskGetTool));
    kernel.register_tool(Box::new(completion::AttemptCompletionTool));
    kernel.register_tool(Box::new(loopal_memory::MemoryTool));
}
