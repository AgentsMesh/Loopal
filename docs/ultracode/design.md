# Loopal Ultracode 设计

状态：V1 已实现；production execution 仍须显式 opt-in，默认关闭。本文记录已落地的 V1
架构、仍受 release latch 保护的执行入口，以及持续适用的发布验收门槛。

## 1. 决策

Loopal 采用 **Hub-owned typed static DAG workflow**：Hub 内的单写者
`WorkflowCoordinator` 持有经验证的静态 workflow graph、child Agent attempt、预算、取消、恢复和
审计因果链；Agent 仍只负责 LLM turn 和 tool execution。

Ultracode 是产品预设，不是 provider effort：

| 轴 | 值 | 含义 |
|---|---|---|
| Model effort | `none..max` | provider 能力，按模型归一化 |
| Orchestration policy | `off / explicit / proactive` | 何时允许规划 workflow |
| Execution form | `direct / workflow` | 当前任务走普通 Agent loop 还是 workflow runtime |

`ultracode` 预设将 policy 设为 `proactive`、选择 Ultracode planner profile，并应用较高但有界的
limits/timing。它为支持该能力的模型推荐 `max` thinking effort，但显式 runtime model/effort 配置可以
覆盖该建议。预设不会设置 `workflow.execution_enabled`：production execution 仍是独立、默认关闭的
release-gated opt-in。它绝不修改 `PermissionMode`、`DecisionMode`、sandbox、cwd 或 secret policy。

## 2. 所有权

```text
Hub
└── WorkflowCoordinator actor        单写者、无 Hub mutex 内 I/O
    ├── WorkflowRun aggregate        graph、revision、limits、terminal result
    ├── Attempt bindings             attempt id -> exact Agent connection generation
    ├── Scheduler                    readiness、admission、deadline、cancel escalation
    ├── Journal                      sole durable writer
    └── Projection publisher         typed events + snapshot summaries

Agent
└── one WorkflowAttempt              普通 LLM/tool loop；不拥有 graph 或 sibling authority
```

Hub 现有 spawn manager、AgentRegistry generation checks、typed `AgentCompletion` 和
`HubMcpService` 是执行底座；Ultracode 不再建第二套 process supervisor、MCP owner 或 permission system。

持久化 WorkflowRun 是对 `docs/architecture.md` 中 O1 “Hub stateless”的窄例外；该例外已由
`docs/architecture.md` 的 O1a（2026-08-08）记录。例外仅限 Hub 内的单写者
`WorkflowCoordinator`：root Session 路径只提供 Hub 推导的存放位置和索引，root Agent 不是 journal
writer。Agent journal + Hub scheduler 会形成 split brain，禁止采用。

## 3. V1 typed graph

V1 不执行任意 JavaScript，也不开放通用 shell/filesystem/network primitive 给 workflow planner。
Planner 只能生成经 Hub 验证的静态 DAG：

```text
WorkflowSpec
  run_goal
  output_node
  nodes[]: AgentNode { id, dependencies, task, worker_profile }
  limits: { max_nodes, max_parallel, max_attempts, run_deadline,
            attempt_timeout, max_text_or_json_bytes }
```

约束：

- node id 唯一，dependency 必须存在，graph 必须无环；V1 只有 all-success join。
- `worker_profile` 是 Hub-owned closed allowlist 中的策略引用；V1 只接受 `default / explore / plan`，
  不能携带 permission/sandbox/cwd/depth override。
- Hub 从可信 parent connection 推导 cwd，`depth = parent.depth + 1`，并对 authority 做交集收缩。
- V1 result 只有 bounded redacted text 或 schema-validated JSON；不提供 generic artifact upload。
- V1 无 dynamic fan-out、condition、loop、pause/replan、cross-Hub 和 parallel writer。
- 自动 retry 只允许发生在 attempt 尚未进入 effectful Running 之前；歧义完成必须人工重试。
- planner 的 canonical JSON Schema 会随 prompt 一起提供；当前不依赖 provider-native structured-output
  enforcement。每次回复仍必须经严格 Serde parse、canonical semantic validation、trusted ceilings 收缩和
  Hub admission validation；解析、schema 或语义校验失败时退回 direct，不产生 workflow effect。

## 4. 身份和状态

不得混用 display name、`AgentId`、`QualifiedAddress`、routing generation 和 attempt id：

- `WorkflowRunId`：持久、opaque。
- `WorkflowNodeId`：spec 内稳定。
- `WorkflowAttemptId`：每次执行新建并持久化；permission、audit、completion 均以它关联。
- `AgentExecutionRef`：Hub 内部 `{address, connection_generation}` lease；不得作为公开 RPC 参数。
- `routing_generation` 只用于本 Hub 的连接隔离，不能充当持久或跨 Hub identity。

Run 状态：`Planned -> Validated -> Running -> Cancelling -> Cancelled`，或从 Running 进入
`Succeeded / Failed`。Node attempt 状态：

```text
Pending -> Ready -> Dispatching -> Running -> Succeeded | Failed
                         \             \-> Cancelling -> Cancelled
                          \-> Failed
dependency failure -> Skipped
```

关键顺序：

1. journal `DispatchIntended(attempt_id)` 后才允许 spawn。
2. spawn registration 必须原子返回 exact child generation，再 journal binding。
3. 只有匹配 run/node/attempt/address/generation 的 `AgentCompletion` 能结束 node。
4. node terminal commit 后才能释放 dependents；run terminal result 最后 commit。
5. cancel 先 commit `Cancelling`、停止 admission，再 interrupt exact leases，超时后 shutdown。
6. slot、attempt count 和 deadline reservation 由 actor 在一次状态转换中完成。
7. command 带 request id；重复请求返回原响应，payload 不同的同 id 请求拒绝。

## 5. 恢复

Journal 是 append-only event log，快照只是加速。Hub restart 后 replay 并重建 projection：

- child start params 持有 `WorkflowAttemptId`，reconnect 时声明该 id；Hub 只接纳 journal 中的 exact attempt。
- 旧 connection generation 的 event/completion/control 一律 quarantine。
- `Dispatching` 且没有确认 spawn 的 attempt 可安全标记失败；不得盲目重复 effectful work。
- `Running` 且未在 recovery grace 内 reclaim exact execution lease 的 attempt 持久化为 `Failed`，failure
  class 为 `AmbiguousExecution`；不得自动重试，后续工作需要显式重新发起。
- graceful Hub shutdown 停止 admission 并持久化状态；不伪装成用户 cancel。

V1 已具备 durable、local-Hub static DAG executor；它只在 `workflow.execution_enabled = true` 且 policy
不是 `off` 时安装并执行。默认配置保持该值为 `false`，因此默认行为仍是非执行性的 direct loop / planner
fallback，而不是自动发布 workflow execution。

## 6. Stage 0 安全发布门槛与已落地边界

以下六类边界已作为 Ultracode V1 的实现与测试范围落地；它们仍是 production execution release gate，
并非因为 workflow 而新引入的风险。

1. **已实施：按连接授权 Hub RPC。** `agent_io/dispatch_loop.rs` 不能把全部 `hub/*`、`meta/*` 暴露给
   任意 Agent。UI、Agent、workflow executor、admin 使用不同 ACL；child 不能直接调用 MCP、secret、
   spawn、任意 target lifecycle 或 Hub shutdown。否则 raw Agent 可切换其他 Agent 的 permission/sandbox，
   或伪造 `MessageSource::Human` 唤醒 suspended session。`Envelope.source` 必须由连接身份生成。
2. **已实施：Hub 推导 spawn authority。** `dispatch/spawn_routing.rs` 不接受 caller 用
   `depth=0`、`bypass`、`no_sandbox`、任意 cwd 扩权。请求入口捕获 requester generation，spawn
   内部 API 原子返回 child generation lease。cross-Hub 因当前无全局 attempt lease 而后移。
3. **已实施：PermissionIntent V2 与 attempt-bound receipts。** 保留现有 fresh、single-use、
   connection-bound interaction token；真正要
   消除的是 approval 之后 pre-hook 仍能改写实际输入。允许的 rewrite 必须在 placeholder input 上先完成，
   再 canonicalize 和 digest 最终 action；pending record 绑定 tool/schema、run/node/attempt、target
   generation 与 UI lease generation，effect boundary 再校验。现有 `(agent, tool)` session grant 是有意的
   产品语义，但不得直接成为持久/retryable workflow grant；V1 禁用或显式收窄它。
4. **已实施：消费者限定 secret。** JIT resolution 后的 plaintext 只进入实际 consumer；post-hook 只看
   placeholder/redacted input。未解析 ref 对 shell/network/MCP effect 必须 fail closed。workflow 无 generic
   `secret/get`。V1 禁止 model-supplied MCP call args 展开 secret；config-owned MCP resolver 由 Hub 在受控边界解析。
5. **已实施：结果末端守卫。** 所有 hook transform 之后，再对 ToolResult、AgentOutput、AgentCompletion、workflow
   result、event/journal/file sink 做 final redaction + size guard。MCP response 使用本次 expansion seed 在
   Hub 内 redaction。secret-exposed 或 unknown-provenance binary/image 默认拒绝持久化。
6. **已实施：protected audit 与 required-audit fail-closed。** production 不得使用 no-op sink；secret、permission、spawn authority、
   attempt lifecycle 和 protected effect 记录 authenticated causation。required audit append 失败时 effect
   不执行；UI event 只是 redacted observation，不替代 protected audit。

恶意 raw-Agent connection 测试覆盖：其无法绕过上述 ACL、伪造 source、复用 stale grant/attempt，
或请求 `depth=0 + bypass + no_sandbox`。

## 7. Protocol 与 projection

已新增并接入以下 concrete-free types 和方法：

- `crates/loopal-protocol/src/workflow/`：ids、spec、state、summary、command、result。
- `hub/workflow/start|get|wait|cancel`：typed request/response；start/cancel 幂等。
- `AgentEventPayload::WorkflowRunChanged`：root-scoped observation，不是执行真相。
- `view/snapshot` 返回 active/recent WorkflowRun summaries；revision gap 继续走现有 resync。

`WorkflowCoordinator` 发布 projection 时不持有 Hub mutex。事件丢失只影响 UI；journal 和 coordinator
state 才是 authoritative。root Agent 收到 terminal notification，但不能自行宣告 workflow 完成。

## 8. 仓库落点

| 已交付范围 | 主要路径 | 已交付能力 |
|---|---|---|
| Stage 0 | `loopal-agent-hub/agent_io`, `dispatch`, `pending_relay`; `loopal-runtime/tool_pipeline`; secret/MCP/audit crates | 六项安全边界、恶意连接和生命周期攻击测试 |
| Stage 1 | `docs/architecture.md`; `loopal-protocol/src/workflow` | O1a durability exception、typed static-DAG schema、纯 reducer/validation |
| Stage 2 | `loopal-agent-hub/src/workflow/{actor,scheduling,journal,recovery}`；root bootstrap | local-Hub executor、durable journal/recovery、hard admission limits、cancel、exact attempt leases |
| Stage 3 | `loopal-view-state`; TUI/ACP/Desktop typed contracts；root-only workflow tools | snapshot/event projection、显式 start/get/wait/cancel、terminal delivery |
| Stage 4 | classifier/planner 与 workflow settings | `proactive` policy、Ultracode preset、direct-vs-workflow decision、runtime model router |

模块按 reason-to-change 拆分；新增或实质重写的 handwritten source/test 文件优先控制在 200 行以内，
超出时必须在变更审查中说明无法进一步按 ownership 拆分的原因。

## 9. 发布验收与持续不变量

- static DAG 可并行执行，依赖只在 durable terminal commit 后释放。
- duplicate command/event/completion、stale generation 和同名 Agent 重连不产生重复 effect 或错误完成。
- cancel、child crash、Hub graceful shutdown、Hub crash/restart 和 truncated journal 有确定结果。
- limits 在并发竞争下不超发 node/attempt/slot；timeout 不靠 LLM 自觉。
- child authority 永不宽于 root/admin/workflow/profile/sandbox 的交集。
- plaintext 不进入 prompt、hook、journal、event、result、overflow、image/resource 或 UI snapshot。
- `proactive` planner 仅在 `workflow.execution_enabled = true` 且 policy 为 `proactive` 时可产生 executable
  workflow；planner 可以决定 `direct`，不以 fan-out 作为成功指标。默认配置和仅选择 `ultracode` preset
  的配置均不自动启用执行。

## 10. 延后事项

cross-Hub execution、parallel writer/worktree、generic artifacts、dynamic fan-out、condition/loop、pause/replan、
reusable permission grants、通用 resource lock service、token/cost hard cap 和任意 workflow scripting 均不在 V1。
其中 parallel writer 最早采用 Hub 创建的“一次 write attempt 一个 exclusive worktree”，不先建设通用 lease manager。
