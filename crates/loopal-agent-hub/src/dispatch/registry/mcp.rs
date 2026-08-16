use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::mcp_handlers::{
    handle_mcp_call_tool, handle_mcp_list_tools, handle_mcp_reconnect, handle_mcp_snapshot,
};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let target = hub.clone();
    let b = b.register_fn(methods::HUB_MCP_LIST_TOOLS.name, move |_params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let agent = agent?;
            crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
            handle_mcp_list_tools(&target, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let target = hub.clone();
    let b = b.register_fn(methods::HUB_MCP_CALL_TOOL.name, move |params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let agent = agent?;
            crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
            handle_mcp_call_tool(&target, params, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let target = hub.clone();
    let b = b.register_fn(methods::HUB_MCP_RECONNECT.name, move |params, ctx| {
        let target = target.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let agent = agent?;
            crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
            handle_mcp_reconnect(&target, params, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    b.register_fn(methods::HUB_MCP_SNAPSHOT.name, move |_params, ctx| {
        let target = hub.clone();
        let agent = crate::dispatch::authorization::agent(ctx);
        Box::pin(async move {
            let agent = agent?;
            crate::dispatch::authorization::revalidate_agent(&target, &agent).await?;
            handle_mcp_snapshot(&target, &agent)
                .await
                .map_err(string_err_to_rpc)
        })
    })
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
