# Stage 2: 把 LocalMcpProvider 从 root agent 搬到 Hub

PR #176 已经铺好了所有需要的抽象：`McpProvider` trait、`McpProxyClient`、`HubMcpClient`、`hub/mcp/*` 协议。但 `LocalMcpProvider` 的**物理位置**还在 root agent 进程内。Stage 2 把它搬到 Hub，并补完 sharing model 和资源标识隔离。

实施 ADR G2/H1/I1/J1/K2。Stage 0 已由 PR #176 完成（non-blocking startup + MCP handshake timeout），本 Stage 不重做。

## 1. 已有抽象（PR #176 留下的，全部保留）

| 组件 | 位置 | 作用 | Stage 2 修改 |
|---|---|---|---|
| `McpProvider` trait | `loopal-mcp/src/provider.rs` | MCP 能力接口（list_tools / call_tool / snapshot） | 不动 |
| `LocalMcpProvider` | `loopal-mcp` | 真正持有 `McpManager` 的实现 | 不动，搬运行位置 |
| `McpProxyClient` | `loopal-mcp` | 通过 IPC 转发的 Provider 实现 | 不动 |
| `HubMcpClient` trait | `loopal-mcp` | IPC 客户端抽象 | 不动 |
| `ConnectionMcpClient` | `loopal-agent-server` | `HubMcpClient` 的 Connection 适配器 | 不动 |
| `hub/mcp/{list_tools,call_tool,snapshot}` | Hub dispatch | IPC 方法名 | 实现改写：不再 forward 到 root，直接调本地 HubMcpService |
| `agent/mcp/*` handler | root agent | Hub forward 的接收端 | **删除** |
| `McpBackend::Local` enum 分支 | `loopal-kernel/src/kernel/mod.rs` | root agent 走 Local，sub 走 Proxy | **删除** |
| `kernel.spawn_mcp()` / `finalize_mcp_tools` | `loopal-kernel` | root agent 启动 MCP 的同步入口 | **删除**（迁移到 Hub） |

## 2. 新增组件

### 2.1 HubMcpService（Hub 进程内的 MCP 拥有者）

`loopal-agent-hub/src/mcp_service.rs`（新文件）：

```rust
pub struct HubMcpService {
    // 多实例：按 cwd 隔离的 hub-singleton 池
    hub_singleton: tokio::sync::RwLock<HashMap<PathBuf, Arc<LocalMcpProvider>>>,
    // per-agent / session 见 Stage 3
    
    secrets: Arc<HubVaultService>,                   // Stage 1 提供
    spawn_registry: Arc<SpawnRegistry>,              // Stage 1 提供
    sampling_router: Arc<SamplingRouter>,            // 见 §4
}

impl HubMcpService {
    pub async fn provider_for(&self, cwd: &Path) -> Arc<dyn McpProvider> {
        // lazy 启动该 cwd 的 LocalMcpProvider；返回 Arc 即可
        // 内部逻辑：读 <cwd>/.loopal/settings.json mcp_servers
        // 过滤 sharing == HubSingleton；自动注入 cwd 资源标识（§3）
        // 用 LocalMcpProvider::spawn_background + wait_until_settled(5s)
        // 启动完成后注册的 ToolAdapter 信息缓存在 LocalMcpProvider 内
    }
}
```

**关键点**：`LocalMcpProvider` **本体代码不动**，只是它的拥有者从 `Kernel`（agent 进程内）变成 `HubMcpService`（Hub 进程内）。

### 2.2 Hub `hub/mcp/*` handler 重写

`loopal-agent-hub/src/dispatch/mcp_handlers.rs`（既有文件，改实现）：

```rust
// 旧（PR #176）
pub async fn handle_mcp_list_tools(hub: &Arc<Mutex<Hub>>) -> Result<Value, String> {
    forward_to_root(hub, methods::AGENT_MCP_LIST_TOOLS.name, json!({})).await
}

// 新（Stage 2）
pub async fn handle_mcp_list_tools(
    hub: &Arc<Mutex<Hub>>,
    caller_agent_id: &str,           // from IPC connection identity
) -> Result<Value, String> {
    let cwd = hub.lock().await.spawn_registry.cwd_of(caller_agent_id)?;
    let provider = hub.lock().await.mcp_service.provider_for(&cwd).await;
    let tools = provider.list_tools().await;
    Ok(serde_json::to_value(McpListToolsResponse { tools: ... })?)
}

// call_tool 加 secret expand（决策 J1）
pub async fn handle_mcp_call_tool(
    hub: &Arc<Mutex<Hub>>,
    caller_agent_id: &str,
    params: Value,
) -> Result<Value, String> {
    let req: McpCallToolRequest = serde_json::from_value(params)?;
    let cwd = hub.lock().await.spawn_registry.cwd_of(caller_agent_id)?;
    
    // Hub 侧 secret expand（决策 J1）
    let allowed = hub.lock().await.mcp_service.secret_eligible_for(&req.server, &req.tool);
    let resolved_args = hub.lock().await.secrets.expand_wire_in_args(
        &cwd, &req.args, &allowed
    ).await?;
    
    // sampling caller-tracking（决策 I1）
    let request_id = generate_request_id();
    hub.lock().await.sampling_router.register_call(request_id.clone(), caller_agent_id);
    
    let provider = hub.lock().await.mcp_service.provider_for(&cwd).await;
    let result = provider.call_tool(&req.server, &req.tool, &resolved_args).await;
    
    hub.lock().await.sampling_router.unregister_call(&request_id);
    Ok(call_result_to_response(&result?))
}
```

### 2.3 删除 root agent 的 `agent/mcp/*` handler

`loopal-agent-server/src/mcp_dispatch.rs` 整文件**删除**。不再有"Hub forward 到 root"这条路径，因为 Hub 自己就是 owner。

`loopal-agent-server/src/connection_mcp_client.rs` **保留**（它的 send_request 现在直接发给 Hub，不变）。

### 2.4 build_kernel_from_config 简化

`crates/loopal-agent-server/src/params.rs`：

```rust
// 旧（PR #176）
pub async fn build_kernel_from_config(
    config: &ResolvedConfig,
    production: bool,
    depth: u32,
    hub_client: Option<Arc<dyn loopal_mcp::HubMcpClient>>,
) -> anyhow::Result<Arc<Kernel>> {
    // ... 设置 secrets ...
    if production {
        if depth > 0 && let Some(client) = hub_client {
            let proxy = loopal_mcp::McpProxyClient::new(client);
            kernel.set_mcp_provider(Arc::new(proxy));
        } else if let Ok(provider) = kernel.resolve_provider(&config.settings.model) {
            // root agent: set sampling on LocalMcpProvider
            let adapter = McpSamplingAdapter::new(provider, ...);
            kernel.set_mcp_sampling(Arc::new(adapter)).await;
        }
        kernel.spawn_mcp().await;
        kernel.finalize_mcp_tools(wait).await;
    }
    // ...
}

// 新（Stage 2）
pub async fn build_kernel_from_config(
    config: &ResolvedConfig,
    production: bool,
    depth: u32,
    hub_client: Arc<dyn loopal_mcp::HubMcpClient>,  // 不再 Option，所有 kernel 必须有
) -> anyhow::Result<Arc<Kernel>> {
    // ... 设置 secrets（Stage 1） ...
    if production {
        // 统一路径：所有 kernel（含 depth=0）都用 Proxy
        let proxy = loopal_mcp::McpProxyClient::new(hub_client);
        kernel.set_mcp_provider(Arc::new(proxy));
        
        // bounded wait: 拉一次 tool list snapshot 注册 ToolAdapter
        // 没拉到的也没关系，Stage 4 的 subscribe_events 会补
        let wait = mcp_startup_wait();
        kernel.finalize_mcp_tools(wait).await;     // 内部走 Proxy.list_tools()
    }
    // ...
}
```

### 2.5 Kernel 字段清理

`loopal-kernel/src/kernel/mod.rs`：

```rust
// 旧
pub(super) enum McpBackend {
    Local(Arc<LocalMcpProvider>),       // ← 删除
    Proxy(Arc<dyn McpProvider>),
}

// 新
// McpBackend enum 整体可删，Kernel 直接持 Arc<dyn McpProvider>
pub struct Kernel {
    // ...
    pub(super) mcp_provider: Arc<dyn McpProvider>,   // 一个字段，统一访问
    // mcp_instructions/resources/prompts 仍然保留（snapshot 一次）
}

// 删除以下方法：
//   - spawn_mcp()
//   - mcp_manager() (返回 Option<Arc<RwLock<McpManager>>>)
//   - local_mcp_provider()
//   - set_mcp_sampling()  ← 改：见 §4 sampling
//   - set_mcp_provider()  ← 改：只在 build_kernel_from_config 时一次性注入
```

## 3. Sharing model + cwd 资源标识自动分配（决策 H1/K2）

`loopal-config/src/settings/mcp.rs` 加 `sharing` 字段（默认 `hub-singleton`）：

```rust
#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum McpSharing {
    #[default]
    HubSingleton,
    PerAgent,    // Stage 3
    Session,     // Stage 3
}
```

`HubMcpService::provider_for(cwd)` 内部按 sharing 过滤后启动 MCP server，并自动注入 cwd 资源标识：

```rust
fn inject_cwd_isolation(server_name: &str, config: &mut McpServerConfig, cwd: &Path) {
    let cwd_id = blake3::hash(cwd.to_str().unwrap().as_bytes()).to_hex()[..16].to_string();
    
    match server_name {
        "chrome-devtools" | "chrome-devtools-mcp" => {
            inject_arg(config, "--user-data-dir",
                home_dir().join(".cache/chrome-devtools-mcp").join(&cwd_id));
        }
        // 后续按需扩展：filesystem-watcher 类似
        _ => { /* 未知 MCP：用户自配置 args 保证 cwd 隔离 */ }
    }
}
```

内置规则表先覆盖 chrome-devtools，工程上扩展 80% 痛点。

## 4. Sampling caller-tracking 路由（决策 I1）

PR #176 通过 `kernel.set_mcp_sampling(adapter)` 让 root agent 的 Provider 处理 MCP server 反向 sampling。搬到 Hub 后，Hub 不能 sampling（它没有 Provider），必须路由回 caller agent。

`loopal-agent-hub/src/sampling_router.rs`（新文件）：

```rust
pub struct SamplingRouter {
    in_flight: tokio::sync::RwLock<HashMap<rmcp::RequestId, AgentId>>,
}

impl SamplingRouter {
    pub fn register_call(&self, id: rmcp::RequestId, caller: AgentId);
    pub fn unregister_call(&self, id: &rmcp::RequestId);
    pub fn lookup(&self, id: &rmcp::RequestId) -> Option<AgentId>;
}
```

Hub 端的 `SamplingCallback` 实现：

```rust
struct HubSamplingDispatcher {
    router: Arc<SamplingRouter>,
    hub: Weak<Mutex<Hub>>,
}

#[async_trait]
impl SamplingCallback for HubSamplingDispatcher {
    async fn create_message(&self, req: SamplingMessageRequest) -> Result<SamplingMessageResponse> {
        // 1. 从 req.request_id 查 caller agent
        // 2. 通过 hub.connection 给该 agent 发反向 IPC: agent/mcp/sampling
        // 3. 等 agent 返回 sampling response
        // 4. 返回给 MCP server
    }
}
```

新增反向 IPC：

```
method: agent/mcp/sampling   (Hub → agent)
params: SamplingMessageRequest
result: SamplingMessageResponse
```

Agent 端的 handler 调本地 Provider 完成 LLM 调用（agent 仍持有 provider_registry）。

## 5. secret_eligible_params 元数据归属（决策 J1）

MCP tool 的 `secret_eligible_params` 元数据迁移到 Hub 侧的 `HubMcpService`：

```rust
struct McpToolRegistration {
    server: String,
    def: ToolDefinition,
    secret_eligible_params: Vec<String>,
}

impl HubMcpService {
    pub fn secret_eligible_for(&self, server: &str, tool: &str) -> Vec<String> {
        // 从 mcp_servers config 的 tool_secret_params 字段读
        // 或从 MCP server tools/list response 的 x-secret-eligible 注解读
        // 默认空数组
    }
}
```

`hub/mcp/call_tool` 在 §2.2 已展示如何用 `secret_eligible_for` + Stage 1 的 `HubVaultService.expand_wire_in_args` 完成 args plaintext 展开，**plaintext 不离开 Hub 进程**。

agent 侧 `McpProxyClient` 传给 Hub 的 args 保留 `<secret_ref:X>` 占位符（不在 agent 内 expand）。

## 6. Agent 端 readiness 协议（订阅工具变更）

PR #176 已经有"late registration"机制：bounded wait 之后慢启动的 MCP server 通过 `register_mcp_tools_for_server` 补注册。但这是 root agent 内的逻辑。

Stage 2 后改为 agent 订阅 Hub 的 MCP 事件流：

```
method: hub/mcp/subscribe_events    (agent → Hub, 长连接)
result: stream<McpEvent>
  ToolsAdded { server, tools: ToolDefinition[] }
  ToolsRemoved { server, tool_names: string[] }
  ServerStatusChanged { server, status }
```

agent 收到 `ToolsAdded` 注册 `HubMcpAdapter`，收到 `ToolsRemoved` 调 `kernel.unregister_tools`。这跟 PR #176 已有的 `register_mcp_tools_for_server` 行为对齐，只是触发源从 root agent 内的 listener 改为 Hub 的 event stream。

## 7. PR 拆分序列（依赖 Stage 1 完成）

1. **PR-12**：`McpSharing` enum + `McpServerConfig.sharing` 字段（不改任何行为）
2. **PR-13**：Hub 内 `HubMcpService` 骨架：拥有 `LocalMcpProvider` per-cwd（无 IPC 接入）
3. **PR-14**：Hub `hub/mcp/list_tools` / `hub/mcp/call_tool` / `hub/mcp/snapshot` handler 改写（不再 forward to root，直接调 HubMcpService）
4. **PR-15**：`SamplingRouter` + `HubSamplingDispatcher` + 反向 `agent/mcp/sampling` 协议
5. **PR-16**：MCP tool args 在 Hub 内 expand secret（依赖 Stage 1 PR-4 的 `secret_runtime`）
6. **PR-17**：cwd 资源标识自动分配（chrome-devtools-mcp 首批规则）
7. **PR-18**：`hub/mcp/subscribe_events` 协议 + Hub 推送事件流
8. **PR-19**：`build_kernel_from_config` 统一走 Proxy（depth=0 也是）
9. **PR-20**：Kernel 字段清理（删 `McpBackend` enum 的 Local 分支、`spawn_mcp` 等方法）
10. **PR-21**：删除 `loopal-agent-server/src/mcp_dispatch.rs`（agent/mcp/* handler 不再需要）
11. **PR-22**：删除 Hub `mcp_handlers.rs` 里的 `forward_to_root` 路径
12. **PR-23**：集成测试 + 端到端 smoke

PR-12 / PR-13 可并行；PR-14 依赖 PR-13；PR-15-17 可并行；PR-18 依赖 PR-13；PR-19 依赖 PR-14；PR-20-22 依赖 PR-19；PR-23 依赖前面全部。

## 8. 测试策略

- **HubMcpService 单元**：mock McpConnection，验证 cwd 隔离 + lazy 启动 + sharing 分支
- **资源标识注入**：chrome-devtools 注入 --user-data-dir 等内置规则的 case
- **SamplingRouter 并发**：register/lookup/unregister 并发正确性
- **MCP args secret expand 在 Hub**：verify plaintext 不出现在 agent 进程的任何位置（grep tracing log）
- **端到端**：mock chrome-devtools-mcp + 真实 Hub + 真实 agent，跨 cwd 启动两个 session，各自有独立 user-data-dir
- **协议兼容性**：sub-agent 同时使用 hub/mcp/call_tool 不被打破（行为跟 PR #176 等价，只是端点从 forward-to-root 变成 Hub 直接处理）

## 9. 验收标准

- `git grep "McpBackend::Local" crates/loopal-kernel/` 返回 0
- `git grep "agent/mcp/" src/ crates/` 仅在 `agent/mcp/sampling` 反向 IPC 出现，其他全部清理
- Root agent 启动后 `ps` 看 chrome-devtools-mcp 的 parent PID 是 Hub 而非 root agent
- Hub crash 后 agent 进入 degraded mode；Hub 重启 + 重连 MCP 后恢复（Stage 4 验收）
- 跨 cwd 两个 session 各自的 chrome-devtools-mcp 用不同的 user-data-dir（`ps aux | grep chrome-profile`）
- `cargo test --workspace` 全绿

## 10. 与 PR #176 的兼容性 / 升级路径

按 §7 协议演进策略（no backward compat）一次性切换。PR-19 / PR-20 是 breaking change：所有 agent 必须升级到新的 Kernel 才能工作。

但 IPC 协议名（`hub/mcp/list_tools` 等）保持不变，所以 TUI / IDE 客户端的代码**不需要改**。这是 PR #176 留下的最大遗产 —— 协议接口已经稳定。
