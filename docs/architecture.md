# Loopal 架构原则

本文档定义 Loopal 的分层职责模型与判准。所有新增模块、跨进程接口、资源持有方的归属决策都必须能从这里推导。原则之间冲突时，靠前的优先级更高。

## 1. Agent / Hub 分层

Loopal 的进程模型分两层：

- **Hub**：资源管理中心。持有所有跨 agent 共享、host-level 唯一、或带 plaintext 生命周期的资源。
- **Agent**：执行单元。处理 LLM turn、tool dispatch、per-session 状态。一个 agent 一个进程，互相隔离。

类比：Hub 是 kubelet（节点资源管理），Agent 是 pod（执行容器）。Hub 长期跑，Agent 是任务级别的生命周期。

## 2. 资源 vs 执行的归属判准

某个对象该归 Hub 还是 Agent，按下列条件依次判断：

1. **是否持有 host-level handle / lock / 长进程？** 是 → Hub。
   - 例：MCP server 子进程、Chrome SingletonLock、Vault 文件句柄、长连接 HTTP client（如带 OAuth 状态）。

2. **是否需要跨 agent 共享或路由？** 是 → Hub。
   - 例：sampling callback 路由（MCP server 反向调 LLM，要回到正确的 agent）、跨 agent 事件分发。

3. **是否承载 plaintext / 敏感数据？** 是 → Hub 拥有源数据，agent 仅 on-demand 短暂获取。
   - 见 §5。

4. **是否是 per-agent / per-session 的执行状态？** 是 → Agent。
   - 例：session messages、cron schedule、background tasks、goal、agent loop state。

5. **是否是声明性的 config / schema？** 是 → Agent own copy。
   - 例：tool registry（schema 而非进程）、settings、hook 配置、provider 定义。

判不出来时，**默认归 Agent**。Hub 职责扩张是单向门，扩进去就不该再缩回来；Agent 误持有资源是常见错误模式，事后能补救。

## 3. 当前归属表（目标态）

| 对象 | 当前持有方 | 目标持有方 | 备注 |
|---|---|---|---|
| `ToolRegistry`（声明） | Kernel (Agent) | Agent | MCP tools 通过 IPC adapter 注册 |
| `McpManager`（进程） | Kernel (Agent) | **Hub** | 重大变更 |
| MCP instructions/resources/prompts | Kernel (Agent) | **Hub**（缓存）+ Agent（snapshot） | |
| `ProviderRegistry`（HTTP client） | Kernel (Agent) | Agent | reqwest 连接池足够，无 host 冲突 |
| `HookService`（shell 命令） | Kernel (Agent) | Agent | 配置驱动，无外部进程持有 |
| `BackgroundTaskStore` | Kernel (Agent) | Agent | per-agent 进程状态 |
| `Vault`（加密存储） | Kernel (Agent) | **Hub** | plaintext on-demand 模型 |
| `Settings` | Kernel (Agent) | Agent own copy | 各 agent 可 override |
| Session storage（messages / cron / goal） | Agent | Agent | per-session 持久化 |
| Sandbox policy | Agent backend | Agent | 执行时策略，非资源 |

## 4. MCP Sharing Model

每个 MCP server 在 config 中声明 `sharing` 字段，决定其生命周期归属。**默认 `hub-singleton`**。

| sharing | 持有方 | 实例数 | 适用场景 |
|---|---|---|---|
| `hub-singleton`（默认） | Hub | 每 cwd 一个（Hub-cwd 内唯一） | 项目本地共享资源：chrome-devtools、filesystem-watcher、git。同 cwd 的所有 agent 共用一个实例；不同 cwd 各自独立 |
| `per-agent` | Hub spawn，attach 到具体 agent | N（每 agent 一份，detach 即 stop） | per-OAuth / per-identity：GitHub MCP、用户身份相关 API |
| `session` | Hub spawn，绑 root agent 生命周期 | 每 root 一个（root 退即停） | project-scoped 工具：local code-search、project memory |

**关键澄清**：`hub-singleton` 的"唯一"是 Hub-cwd 内唯一，不是 host-physical 唯一。host-level 真单例资源（如 chrome 的 user-data-dir）由 Hub 在 spawn 时为每个 cwd 自动分配独立资源标识（例如 `--user-data-dir=~/.cache/chrome-devtools-mcp/<cwd_hash>/`），避免 host-level 锁冲突。

判准：默认 `hub-singleton` 对大多数 MCP 都正确；选 `per-agent` 必须有明确的权限/状态隔离需求；`session` 是中间档，少用。

Agent 进程**永远不直接管理 MCP 进程**。Agent 通过 `hub/mcp_*` IPC 列工具、调工具。MCP server 的 secret 展开在 Hub 内完成，agent 拿到的工具 schema 中不含 plaintext。

**`secret_eligible_params` 双归属**：每个 tool 声明 `secret_eligible_params` 描述哪些 args 字段允许 expand `<secret_ref:X>`。归属规则：

- MCP tool 的 metadata 由 Hub 持有（`McpManager` 注册时缓存），expand 在 Hub 内完成
- Builtin tool（Bash、Edit 等）的 metadata 由 agent 持有，expand 在 agent 内完成
- 不存在"统一"的注册中心 —— tool 跟它的元数据归属同一个进程

## 5. Plaintext 与敏感数据生命周期

**Vault 在 Hub**。Agent 进程**不持有** Vault store。

### 5.1 plaintext-on-demand 协议

Agent 需要 plaintext 时（Bash tool env、provider api_key 等）：

1. Agent 识别本地模板（`{{secret:NAME}}` author 或 `<secret_ref:NAME>` wire）中的占位符。
2. 对每个 NAME 调 `hub/secret_get(name)` IPC，Hub 返回 `Zeroizing<String>` plaintext。
3. Agent 在本地完成模板替换（拼接出最终字符串）。
4. Agent **立即消费**：写入 child process env / 发起 HTTP 请求 / 立刻 spawn 子进程。
5. 消费完成后所有 `Zeroizing<String>` drop，内存自动 zero 化。

约束：plaintext 在 agent 内存的生命周期短于一次 IPC roundtrip + 一次消费动作。任何持有更久的设计必须改方案。

### 5.2 Rust 类型强约束

- Agent 端 plaintext 必须以 `Zeroizing<String>` 类型流转。drop 时自动 zero memory。
- `Debug` impl 输出 `[REDACTED]`，避免意外进 tracing。
- clippy lint 禁止在 plaintext 路径上的 `.clone()` / `.to_string()` 隐式分配。
- 任何把 `Zeroizing<String>` 转成普通 `String` 的代码必须在 PR 中显式说明理由。

### 5.3 跨进程边界规则

**plaintext 不通过 IPC 流转**，除非接收端就是它的实际消费者：

- Bash tool spawn child env：agent 在本地展开（agent 是消费者），plaintext 不上 Hub。
- MCP tool args 展开：agent 把含 `<secret_ref:X>` 的 args 发给 Hub，**Hub 在内部展开**（Hub 是 MCP server 的进程父）；plaintext 不回 agent。
- LLM provider api_key：每次请求前 agent 拉一次 secret 到本地，注入 HTTP header，请求完 drop（agent 是消费者）。

### 5.4 Sub-agent vault 访问限制

Sub-agent 通过 `cwd_override` 可在不同目录工作，但 vault 访问受**祖先链校验**：

- Hub 维护 spawn 父子关系图（`SpawnParams.parent_id`）。
- `hub/secret_get` 调用时校验 caller 的 cwd 在 spawn root 的 cwd 树内（祖先或后裔）。
- 跨树访问拒绝（返回 `PermissionDenied`），即使 sub-agent 显式声明了其他 cwd 也不行。
- 防止 sub-agent 作为跨项目权限放大的踏板。

### 5.5 Audit

每次 `hub/secret_get` 一条 audit log，由 **Hub 进程**写入 `~/.loopal/telemetry/secret_access.jsonl`，权限 0600。字段：timestamp、cwd、secret_name、caller agent_name、caller depth、caller tool_name（如可推断）。Agent 进程不再写此文件。

LLM 看到的占位符：`<secret_ref:NAME>`（wire 格式）。Author 格式 `{{secret:NAME}}` 用于配置文件，在到达 LLM 或子进程之前全部被翻译/展开。两种格式的 NAME 都遵循 `[a-z][a-z0-9_]*` 正则。

## 6. 启动语义：Liveness vs Readiness

`agent/start` IPC 的返回不等于"全量就绪"。明确区分：

- **Liveness**（`agent/start` 返回）：进程跑起来了，agent loop 在 select 输入，能接 message。仅此而已。
- **Readiness**（事件流式 emit）：能力逐项就绪。MCP server connect 完成时 Hub emit `mcp/tools_added`，agent registry 动态扩张。

规则：
- `agent/start` 的同步路径上**不允许有任何"等外部依赖连上"**的逻辑。
- 外部资源（MCP、Vault decrypt、远端 config）必须 fire-and-forget，状态机驱动，渐进通知。
- LLM 每个 turn 重新读 `tool_definitions()`——架构已经支持，protocol 层就该利用。

## 7. 协议演进策略

**No backward compat**：协议变更等于协议替换。

不维护 mixed-version：升级时所有 agent / Hub / TUI 一起切换。新协议方法（如 `hub/mcp_*`、`hub/secret_get`）不与旧字段共存，移除即移除。

例外仅在跨 host 集群（不同 Hub 节点）的场景成立，且必须在 PR 中显式说明跨版本兼容窗口。

## 8. 故障模型

- **Hub 是单点**。Hub 挂掉，所有 agent 失去 MCP / Vault 能力。Agent 必须 graceful degrade：MCP IPC 失败 fall back 到 builtin tools，对 LLM 暴露 "MCP unavailable" 状态而非 hang。
- **MCP server 故障不传染**：单个 MCP server crash 只影响其工具，Hub 标记该 server `Failed`，其他 MCP 与 agent loop 继续运行。
- **Agent 进程 crash 是预期事件**：sub-agent 设计成可重启、可被 kill。Hub 是 supervisor，按策略重 spawn 或上报 root agent。

## 9. 落地阶段

| 阶段 | 范围 | 工期估算 |
|---|---|---|
| 0 | ✅ 已完成（PR #176 adcbfd03）：MCP handshake outer timeout；non-blocking startup；sub-agent 通过 `McpProxyClient` 访问 root 的 MCP；late registration listener；`McpProvider` trait + `HubMcpClient` 抽象铺好 | — |
| 1 | Vault 上 Hub；`hub/secret_get(name)` IPC；agent 端模板拼装 + `Zeroizing<String>`；Provider api_key plaintext-on-demand；sub-agent cwd 祖先链校验 | 1-1.5 周 |
| 2 | 把 `LocalMcpProvider` 物理位置从 root agent 搬到 Hub；删 `McpBackend::Local`；所有 agent 统一走 Proxy；`hub-singleton` cwd-aware 多实例 + 资源标识自动分配；sampling caller-tracking；MCP args secret 在 Hub 侧 expand | 1-2 周 |
| 3 | `per-agent` / `session` sharing 完整化；attach 时 spawn / detach 时 stop | 0.5-1 周 |
| 4 | Hub IPC graceful degradation（builtin only fallback）；Hub stateless 重启路径 | 0.5 周 |

阶段之间不允许跳跃：Stage 1 完成且原则在代码里固化后才能开 Stage 2。每个阶段一个独立 PR 集，落地后回写本文档对应的"当前归属表"。

## 10. 决策记录格式

未来对本文档的修改应作为 Architecture Decision Record（ADR）提交，新增章节或修改归属表的 PR 描述中必须包含：

- 触发本次决策的具体场景（bug、性能、新功能）
- 为什么旧归属不再适用
- 新归属如何满足 §2 的判准
- 是否产生协议 breaking change，迁移路径

新决策追加到 §11 决策表底部，每条带日期、范围、决策、备注。

## 11. 决策表

| ID | 日期 | 范围 | 决策 | 备注 |
|---|---|---|---|---|
| A1 | 2026-05-20 | Vault 权威性 | Vault file 是 ground truth；CLI 直接读写文件，不经 Hub | bootstrap 时 Hub 没起来也能修 vault |
| B1 | 2026-05-20 | Vault 定位 | Hub 按 cwd 动态多 vault；`HashMap<cwd, AgeVault>` lazy 加载 | 跨 cwd 的 agent 看到各自 vault |
| C2 | 2026-05-20 | plaintext 类型 | 全程 `Zeroizing<String>`，drop 自动 zero memory | clippy lint 防隐式分配 |
| D2 | 2026-05-20 | secret 接口粒度 | Hub 只提供 `secret_get(name)` 原子接口；agent 内做模板替换 | 协议简洁优先；agent 内拼装走 `Zeroizing<String>` |
| E1 | 2026-05-20 | sub-agent vault | Sub-agent 限制同树访问；跨树拒绝 | Hub 维护父子关系 + cwd 祖先链校验 |
| F2 | 2026-05-20 | Provider api_key | Stage 1 同时改 Provider api_key 为 plaintext-on-demand | 每次 LLM 请求前拉、用完 drop |
| G2 | 2026-05-20 | MCP 启动时机 | Lazy per-cwd，agent attach 时启动该 cwd 的 MCP server | Hub 启动快；首个 agent 慢 |
| H1 | 2026-05-20 | MCP config 归属 | MCP config 也 per-cwd（跟 vault 一致） | 同 cwd 共享 MCP 实例集 |
| I1 | 2026-05-20 | sampling 路由 | Hub 拦截 rmcp RequestId，路由回 caller agent 的 Provider | wrap MCP transport 跟 correlation |
| J1 | 2026-05-20 | MCP tool args secret | agent 传 `<secret_ref>` 占位符，Hub 内 expand | plaintext 不离开 Hub |
| K2 | 2026-05-20 | hub-singleton 范围 | Hub-cwd 内唯一（不是 host-physical 唯一） | Hub 为每 cwd 分配独立资源标识 |
| L1 | 2026-05-20 | per-agent 实例生命周期 | agent detach 立即 stop 实例，不池化 | 简单清晰，无状态污染 |
| N1 | 2026-05-20 | Hub 不可用 | Graceful degrade：builtin tool 正常；MCP/secret 返回错误给 LLM | LLM 能决策是否重试 |
| O1 | 2026-05-20 | Hub crash recovery | Hub 是 stateless service，重启重新加载 cwd vault + 重连 MCP | 不持久化 runtime 状态 |
