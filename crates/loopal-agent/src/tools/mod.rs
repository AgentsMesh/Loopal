pub mod agent;
pub mod channel;
pub mod completion;
pub mod send_message;
pub mod task;
pub mod worktree;

use loopal_kernel::Kernel;

/// Register all agent tools into the kernel.
///
/// Tools are stateless structs; they access `AgentShared` at runtime
/// via `ToolContext.shared` downcast, not at registration time.
pub fn register_all(kernel: &mut Kernel) {
    kernel.register_tool(Box::new(agent::AgentTool));
    kernel.register_tool(Box::new(send_message::SendMessageTool));
    kernel.register_tool(Box::new(channel::ChannelTool));
    kernel.register_tool(Box::new(task::TaskCreateTool));
    kernel.register_tool(Box::new(task::TaskUpdateTool));
    kernel.register_tool(Box::new(task::TaskListTool));
    kernel.register_tool(Box::new(task::TaskGetTool));
    kernel.register_tool(Box::new(completion::AttemptCompletionTool));
    kernel.register_tool(Box::new(worktree::EnterWorktreeTool));
    kernel.register_tool(Box::new(worktree::ExitWorktreeTool));
    kernel.register_tool(Box::new(loopal_memory::MemoryTool));
}
