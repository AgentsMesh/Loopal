# Stage 3: per-agent / session sharing 完整化

实施 ADR L1（detach 立即 stop）+ 完成 sharing model 三档。

依赖 Stage 2 完成：`HubMcpService` + `McpSharing` enum + `hub/mcp/*` 协议已就位。

## 1. 范围

Stage 2 只实现了 `hub-singleton`。Stage 3 补完：

- `per-agent`：每个 agent attach 时 Hub spawn 独立实例；agent detach（exit/crash）立即 stop
- `session`：root agent attach 时启动；root agent 退出时 stop。子 agent 共用 root 的实例

## 2. 实例生命周期状态机

```
                  attach + sharing match
   Disconnected ────────────────────────► Connecting
                                             │
                                connect ok   │ connect fail
                                  ┌──────────┴──────────┐
                                  ▼                     ▼
                              Connected               Failed
                                  │                     │
              owner detach        │       retry on next attach
                                  ▼                     │
                              Stopping ─────────────────┘
                                  │
                                  ▼
                              Disconnected
```

`owner detach` 触发条件按 sharing 不同：

- `hub-singleton`：cwd 内最后一个 agent detach 时 stop（Stage 2 已实现但实际策略待定 —— L1 倾向于保留实例，下次 attach 复用；如果偏向更激进 GC 可改成立即停）
- `per-agent`：owner agent detach 立即 stop
- `session`：owner = root agent detach 时 stop（**整个 session 内所有 sub-agent 都已 disconnect 才算 root detach**）

## 3. HubMcpService 数据结构扩展

`loopal-agent-hub/src/mcp_manager.rs`：

```rust
pub struct HubMcpService {
    hub_singleton: tokio::sync::RwLock<HashMap<PathBuf, CwdMcpSet>>,
    per_agent: tokio::sync::RwLock<HashMap<AgentId, CwdMcpSet>>,
    session: tokio::sync::RwLock<HashMap<AgentId /* root */, CwdMcpSet>>,
    
    // 反向索引：从 sub-agent 找 root（用于 session sharing lookup）
    sub_to_root: tokio::sync::RwLock<HashMap<AgentId, AgentId>>,
    
    secrets: Arc<HubVaultService>,
    sampling_router: Arc<SamplingRouter>,
}
```

attach 时的处理（在 Stage 2 基础上扩展）：

```rust
async fn on_agent_attach(&self, agent_id: AgentId, cwd: PathBuf, parent: Option<AgentId>) {
    let config = load_mcp_config(&cwd);
    
    for (name, server_config) in config.mcp_servers {
        match server_config.sharing() {
            HubSingleton => self.ensure_hub_singleton(cwd.clone(), name, server_config).await,
            PerAgent => self.spawn_per_agent(agent_id.clone(), name, server_config).await,
            Session => {
                let root = self.find_root(&agent_id, &parent);
                self.ensure_session(root, name, server_config).await
            }
        }
    }
}
```

`find_root` 沿 parent 链向上找到 `parent == None` 的 agent_id（spawn_registry 提供）。

detach 时的处理：

```rust
async fn on_agent_detach(&self, agent_id: AgentId) {
    // per-agent: 立即停
    if let Some(set) = self.per_agent.write().await.remove(&agent_id) {
        set.stop_all().await;
    }
    
    // session: 如果 agent_id 是 root 且 sub-agent 已全部 disconnect → 停
    if self.is_root_with_no_subs(&agent_id) {
        if let Some(set) = self.session.write().await.remove(&agent_id) {
            set.stop_all().await;
        }
    }
    
    // hub-singleton: cwd 内最后一个 agent → 停
    let cwd = self.spawn_registry.cwd_of(&agent_id);
    if self.no_agents_in_cwd(&cwd) {
        if let Some(set) = self.hub_singleton.write().await.remove(&cwd) {
            set.stop_all().await;
        }
    }
    
    self.sub_to_root.write().await.remove(&agent_id);
}
```

## 4. tool list 在不同 sharing 下的可见性

`hub/mcp/list_tools` 在 Stage 2 返回 cwd 的所有 hub-singleton tool。Stage 3 扩展为按 agent 视角返回：

```rust
async fn list_tools_for(&self, agent_id: AgentId, cwd: PathBuf) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    
    // 1. hub-singleton: cwd 共享池
    if let Some(set) = self.hub_singleton.read().await.get(&cwd) {
        tools.extend(set.all_tools());
    }
    
    // 2. per-agent: 仅自己的
    if let Some(set) = self.per_agent.read().await.get(&agent_id) {
        tools.extend(set.all_tools());
    }
    
    // 3. session: 自己的 root 拥有的
    let root = self.find_root_of(&agent_id);
    if let Some(set) = self.session.read().await.get(&root) {
        tools.extend(set.all_tools());
    }
    
    tools
}
```

注意：sub-agent 默认**不继承** root agent 的 per-agent MCP（per-agent 的"agent"指的是某个具体 agent，不传给 sub）。这是 per-agent sharing 的语义。如果需要 sub 也能用，应该把 sharing 改成 `session`。

## 5. hub/mcp/call_tool 路由扩展

Stage 2 只查 hub-singleton。Stage 3 按 sharing 查正确的 connection：

```rust
async fn call_tool(&self, caller: AgentId, cwd: PathBuf, server: String, ...) -> ... {
    // 按优先级查 connection：
    // per-agent (自己的) → session (root 的) → hub-singleton (cwd 的)
    let conn = self.per_agent.read().await
        .get(&caller).and_then(|s| s.get_server(&server))
        .or_else(|| {
            let root = self.find_root_of(&caller);
            self.session.read().await.get(&root).and_then(|s| s.get_server(&server))
        })
        .or_else(|| self.hub_singleton.read().await.get(&cwd).and_then(|s| s.get_server(&server)))
        .ok_or(McpError::ServerNotFound)?;
    
    // 后续 secret expand + call_tool 同 Stage 2
}
```

## 6. Spawn 时的 per-agent / session 触发

`SpawnParams` 在 Stage 1 已带 cwd / parent_id。Stage 3 额外让 Hub spawn_and_register 流程触发 per-agent / session MCP spawn：

```rust
// loopal-agent-hub/src/spawn_manager/spawn.rs

async fn spawn_and_register(..., agent_id: String, cwd: PathBuf, parent: Option<String>) {
    // 既有：fork process, init IPC, agent/start
    // ...
    
    // 新增：注册到 spawn_registry（Stage 1 已加）
    spawn_registry.register(agent_id.clone(), cwd.clone(), parent.clone());
    
    // 新增：触发 sharing-aware MCP attach
    hub_mcp_service.on_agent_attach(agent_id, cwd, parent).await;
}
```

agent disconnect / shutdown 时：

```rust
async fn on_disconnect(&self, agent_id: AgentId) {
    spawn_registry.unregister(&agent_id);
    hub_mcp_service.on_agent_detach(agent_id).await;
}
```

## 7. 配置示例

```jsonc
// <project>/.loopal/settings.json
{
  "mcp_servers": {
    // 默认：hub-singleton，per-cwd 共享
    "chrome-devtools": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "chrome-devtools-mcp@latest"]
    },
    
    // 显式 per-agent：每个 agent 独立 GitHub token
    "github": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@github/mcp"],
      "env": { "GITHUB_TOKEN": "{{secret:github_token}}" },
      "sharing": "per-agent"
    },
    
    // 显式 session：跟 root agent 同生命周期
    "code-search": {
      "type": "stdio",
      "command": "./bin/code-search-mcp",
      "sharing": "session"
    }
  }
}
```

## 8. PR 拆分序列

1. **PR-22**：`HubMcpService` 三个 HashMap + state machine（无 IPC 改动）
2. **PR-23**：`on_agent_attach` / `on_agent_detach` 实现 + per-agent 路径
3. **PR-24**：session sharing 路径 + `find_root_of` + `sub_to_root` 索引
4. **PR-25**：`list_tools_for(agent)` 视角合并
5. **PR-26**：`call_tool` 按 sharing 优先级查 connection
6. **PR-27**：Hub spawn_and_register 接入 attach 触发
7. **PR-28**：disconnect 触发 detach
8. **PR-29**：集成测试（三种 sharing 各一个 case）

## 9. 测试策略

- **状态机单元测试**：mock 时钟，验证 attach/detach 序列下的 connection 状态正确
- **find_root 算法**：spawn registry mock，验证多层 sub-agent 找 root 正确
- **三种 sharing 并存**：同一 cwd 同时配 hub-singleton + per-agent + session，验证 tool list 正确聚合
- **session lifecycle**：root → spawn child A → child A spawn grandchild B → A 退 → B 退 → root 退；session MCP 直到 root 退才停
- **per-agent 隔离**：两个 agent 都用 github MCP，验证 token 不串

## 10. 验收标准

- `git grep "sharing" crates/loopal-config/src/settings/mcp.rs` 确认字段定义
- 三种 sharing 都能跑 smoke test
- `cargo test --workspace` 全绿
- L1 决策落地：per-agent MCP 在 agent detach 后通过 `ps` 查不到对应进程（在 grace period 后）
