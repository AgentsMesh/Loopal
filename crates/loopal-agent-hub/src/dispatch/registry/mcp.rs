use std::sync::Arc;

use loopal_ipc::DispatcherBuilder;
use loopal_ipc::protocol::methods;
use tokio::sync::Mutex;

use crate::dispatch::mcp_handlers::{
    handle_mcp_call_tool, handle_mcp_list_tools, handle_mcp_snapshot,
};
use crate::hub::Hub;

use super::string_err_to_rpc;

pub fn register(b: DispatcherBuilder, hub: Arc<Mutex<Hub>>) -> DispatcherBuilder {
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_MCP_LIST_TOOLS.name, move |_params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            handle_mcp_list_tools(&h, &from)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    let b = b.register_fn(methods::HUB_MCP_CALL_TOOL.name, move |params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            handle_mcp_call_tool(&h, params, &from)
                .await
                .map_err(string_err_to_rpc)
        })
    });
    let h = hub.clone();
    b.register_fn(methods::HUB_MCP_SNAPSHOT.name, move |_params, ctx| {
        let h = h.clone();
        let from = ctx.from.clone();
        Box::pin(async move {
            handle_mcp_snapshot(&h, &from)
                .await
                .map_err(string_err_to_rpc)
        })
    })
}
