mod cancel;
mod common;
mod get;
mod schema;
mod start;
mod wait;

use loopal_kernel::Kernel;

pub fn register(kernel: &Kernel) {
    kernel.register_tool(Box::new(start::WorkflowStartTool));
    kernel.register_tool(Box::new(get::WorkflowGetTool));
    kernel.register_tool(Box::new(wait::WorkflowWaitTool));
    kernel.register_tool(Box::new(cancel::WorkflowCancelTool));
}

#[cfg(test)]
mod tests;
