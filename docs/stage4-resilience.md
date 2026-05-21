# Stage 4: 运行时弹性

实施 ADR N1（graceful degrade builtin only）+ O1（Hub stateless 重启路径）。

依赖 Stage 1-3 完成。本 Stage 不引入新协议，只补 IPC 不可用时的行为模型 + Hub 重启恢复。

## 1. 范围

- Hub IPC 失败 / 慢 / 断开时，agent 的标准 fallback 行为
- Hub crash / 重启后，agent 自动重连 + 重建 MCP/Vault 视图
- 不引入持久化（O1：Hub 仍是 stateless service）

## 2. IPC 错误分类

`loopal-secret-client` 和 agent 端的 HubMcpAdapter 共用同一套错误分类：

```rust
pub enum HubIpcError {
    // 临时性，建议重试
    Transient(TransientCause),
    
    // 永久性，本次调用失败但 Hub 仍正常
    Permanent(PermanentCause),
    
    // Hub 完全不可达 → 进入 degraded mode
    Unavailable,
}

pub enum TransientCause {
    Timeout,            // 单次请求超时（如 2s）
    QueueFull,          // Hub 排队满
}

pub enum PermanentCause {
    NotFound,           // secret/tool 不存在
    PermissionDenied,   // sub-agent 越权
    InvalidArgs,
    InternalError(String),
}
```

`Unavailable` 触发条件：连续 N 次 `Transient` 失败（默认 N=3），或 IPC 连接 socket 级断开。

## 3. Agent 端 degraded mode

```rust
pub struct DegradedState {
    is_degraded: AtomicBool,
    degraded_since: parking_lot::Mutex<Option<Instant>>,
    mcp_tool_snapshot: parking_lot::RwLock<Vec<HubMcpAdapter>>,  // 缓存上次的 tool list
}
```

进入 degraded mode 时：

1. `is_degraded.store(true)`
2. `agent_event::HubDegraded { since }` emit 给 UI
3. tool_registry 保留 builtin tool（Read/Write/Edit/Bash/Grep 等）
4. MCP tool 从 registry 中**临时移除**（避免 LLM 调用一个保证失败的工具）
5. Provider 收到 secret_client 失败 → 提示 LLM "credentials unavailable，请重试"

恢复条件：

1. 后台 reconnect task 每 1s 试一次 `hub/secret_health`
2. 第一次成功 → `is_degraded.store(false)`，重新拉 `hub/mcp/list_tools`
3. emit `HubRecovered { duration }`

## 4. Tool 调用时的错误传播

LLM 看到的 ToolResult 应该清晰区分三类错误：

```rust
// 当前 ToolResult 就有 is_error: bool + content
// 增强：用 ToolResult content 的结构化错误

// MCP tool 调用 → Hub Unavailable
{ "is_error": true, "content": "[hub_unavailable] MCP tool '{name}' temporarily unavailable. \
                               Try again or use builtin alternatives." }

// Bash tool 中 secret expand → Hub Unavailable  
{ "is_error": true, "content": "[secret_unavailable] Secret '{NAME}' cannot be resolved \
                               while Hub is unavailable." }

// Provider api_key 失败
{ "is_error": true, "content": "[provider_auth_unavailable] LLM provider credentials \
                               temporarily unavailable. Conversation paused." }
```

`[hub_unavailable]` / `[secret_unavailable]` / `[provider_auth_unavailable]` 是稳定的前缀标记，方便 LLM 识别这是 fail-safe 信号而非业务错误，决定是否重试或绕过。

## 5. Provider auth fail 时的 agent loop 处理

LLM 请求前 `provider.auth_header().await` 失败时：

- **非 degraded mode + Transient**：内部重试 3 次，间隔 200ms / 400ms / 800ms。成功则继续；全失败则升级为 Permanent。
- **Permanent / Degraded**：不 retry。本次 turn 中止，emit `AgentEvent::TurnAborted { reason: "provider_auth_unavailable" }` 给 UI。input_rx 回到 awaiting 状态。

**关键**：不 hang。每个 IPC 调用都有超时（默认 2s），失败立即明确表达，让 LLM / UI / 上游能决策。

## 6. Hub 重启路径（O1）

Hub 进程崩溃或被重启时：

1. agent 的 IPC socket 收到 EOF → IPC layer emit ConnectionLost
2. agent 进入 degraded mode（§3）
3. agent 后台 reconnect task 尝试连接 Hub 的固定监听地址（unix socket / TCP port）
4. Hub 重启后：
   - 清空所有 in-memory state（vault cache、MCP connections、spawn registry、sampling router）
   - 重新读 `{tmp}/loopal/run/<pid>.json`，开 listener
5. agent reconnect 成功 → 重新执行 attach 序列：
   - 声明 cwd / agent_name / parent_id
   - `hub/mcp/list_tools` 拉 snapshot
   - `hub/mcp/subscribe_events` 重订
6. Hub 那边按 §2 lazy 启动该 cwd 的 hub-singleton MCP（首个 agent 到达）+ 该 agent 的 per-agent MCP（如配置了的话）
7. `is_degraded.store(false)`

注意：Hub 没有持久化 spawn registry，所以 agent reconnect 时必须**重新声明**自己的 cwd 和 parent。这要求 agent 端记住自己 spawn 时的 parameters（已经有了，在 `AgentShared` 内）。

**孤儿 MCP server**：Hub 重启时，之前 spawn 的 MCP 子进程仍然在运行（没 receive 到 kill）。Hub 不知道它们存在 → 第二次启动相同 MCP 时会 spawn 第二个实例。这是个潜在的资源泄漏。

缓解：MCP child process 应该是 Hub 进程的直接子进程，Hub 退出时它们也会被 SIGHUP（macOS 下需要 `ProcessGroup::kill_on_drop` 或类似机制）。验证：Hub crash 测试中 spawn 的 chrome-devtools-mcp 进程是否随 Hub 一起退。

## 7. UI / TUI 反馈

新增 protocol 事件：

```
AgentEvent::HubDegraded   { since: u64 }
AgentEvent::HubRecovered  { duration_ms: u64 }
```

TUI 处理：

- HubDegraded → 顶栏显示红色 "Hub disconnected — builtin tools only" + 时间
- HubRecovered → 短暂显示绿色 "Hub recovered (xxx ms)"，几秒后自动消失
- 期间 LLM 输入不锁，可以继续；只是某些 tool call 会被 LLM 直接 fail-fast

## 8. PR 拆分序列

1. **PR-30**：`HubIpcError` 分类 + 重试策略（在 `loopal-secret-client` 内）
2. **PR-31**：`DegradedState` 数据结构 + tool_registry 临时 unregister MCP tools 路径
3. **PR-32**：reconnect 后台 task + `hub/secret_health` 探活
4. **PR-33**：Provider 端的 retry / abort 逻辑
5. **PR-34**：`AgentEvent::HubDegraded` / `HubRecovered` 事件协议
6. **PR-35**：TUI 显示 degraded 状态
7. **PR-36**：Hub child process kill_on_drop / process group 处理
8. **PR-37**：集成测试（kill Hub → 验证 agent degrade → 重启 Hub → 验证 recovery）

## 9. 测试策略

- **错误分类单元**：mock IPC 错误，验证 Transient/Permanent/Unavailable 分类正确
- **重试退避**：注入间歇性 timeout，验证重试次数 + 间隔
- **degraded mode tool registry**：进入 degraded → MCP tools 不在 list 中；recover → 重新出现
- **Provider abort 测试**：mock SecretClient 永久失败，验证 LLM turn 被 abort 而不是 hang
- **Hub restart 端到端**：spawn 真实 Hub + agent，kill Hub PID，观察 agent 进入 degraded，重启 Hub，观察 recover
- **孤儿 MCP 验证**：上一步测试中观察 chrome-devtools-mcp 子进程是否随 Hub 一起退

## 10. 验收标准

- 杀掉 Hub 进程后 2s 内 agent 进入 degraded mode（不 hang）
- Hub 重启后 5s 内 agent 自动恢复 + MCP tools 重新可用
- degraded 期间 builtin tool 正常工作，LLM 能继续对话
- LLM 收到的 tool error 含 `[hub_unavailable]` / `[secret_unavailable]` 前缀
- `ps` 检查 Hub crash 后不留 chrome-devtools-mcp 孤儿进程
- `cargo test --workspace` 全绿
- TUI 顶栏正确显示 Hub 状态切换
