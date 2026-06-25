# Glob Traversal Hang — Post-Review Consolidation

状态：已实现。本文记录 0001(P0)/0002(P1) 落地后，一次 max-effort code review 驱动的修正。取代 [0001](0001-root-cause-analysis.md) §5.3 的"后端外层硬兜底"设计。

| | |
|---|---|
| **触发** | 对 P0+P1 diff 的 10-angle code review |
| **核心发现** | 后端自建的外层 `tokio::time::timeout` backstop 与 runtime 已有的 `watchdog_deadline` 通用机制**重复**，且只覆盖 glob/grep；`truncated` 与超时**语义混淆**导致超时写空 overflow 文件 |
| **范围** | `loopal-backend/src/search/`、`loopal-tool-api`、`loopal-runtime` watchdog、glob/grep 工具层 |

---

## 1. 背景：评审发现了什么

P0([0001](0001-root-cause-analysis.md))为 glob/grep 加了**双层**超时：内层协作式 `walk_timeout` + 后端外层 `tokio::time::timeout(walk_timeout+10s)`。评审指出：

1. **重复造轮子（altitude）**：runtime 早有通用 per-tool 看门狗 `tool_watchdog::watchdog_deadline()`（`tool_exec.rs` 对其返回 `Some` 的工具套 `tokio::time::timeout`，超时返回**带 typed `StaleReason::WatchdogTimeout`** 的结果）——但它**只对 Bash 生效**，其余工具返回 `None`。P0 没有扩展它，而是在后端另起一套外层超时，且只保护 glob/grep。原始事故类（"任意慢 IO 挂死 agent loop"）只关了一半：**Ls / Read 走死挂载依旧会挂**。
2. **`truncated` 语义混淆**：glob/grep 的 `truncated = done.load()`，而 `done` 同时被"命中 max"和"超时"置位，导致**每次超时都误判为截断并写一个无人读取的 overflow 文件**（工具层只读 `entries`/`timed_out`）。
3. **`Err(Timeout)` 误处理**：后端外层超时返回 `Err(ToolIoError::Timeout)`，被工具 `execute` 的 `.map_err(...)?` 转成硬错误，绕过了精心设计的 `timed_out` 部分结果 + 提示路径。
4. 次要：`TIMEOUT_NOTICE` 在两个工具里逐字重复；glob 的 `count` 原子与 `entries.len()` 冗余；`walk_timeout` doc 注释夸大"dead NFS"保证；两处 grace 常量（10s vs watchdog 30s）。

## 2. 合并后的分层（取代 0001 §5.3）

```
内层（后端，协作式）  ResourceLimits::walk_timeout = 30s
  └─ 在 walk 的两次 entry 之间检查 Instant >= deadline
  └─ 命中 → 返回【部分结果】+ timed_out=true（Ok），工具层追加 SEARCH_TIMEOUT_NOTICE 文本
  └─ 局限：打不断卡在单个 readdir/stat syscall 的线程

外层（runtime，硬兜底）  tool_watchdog::watchdog_deadline = 60s
  └─ 收敛点 tool_exec::execute_tool_watchdogged 对 Bash + 7 个 fs-read 工具
     (Glob/Grep/Ls/Read/ReadPdf/ReadImage/ReadHtml) 套 tokio::time::timeout
  └─ 命中 → 返回带【typed StaleReason::WatchdogTimeout】的 is_error 结果，解放 agent loop
  └─ 覆盖 Ls/Read 等（P0 没覆盖的死挂载向量）
```

正常慢树：内层 30s 先返回部分结果，外层 60s 永不触发。死挂载：内层打不断，外层 60s 兜底。后端**不再**自建外层超时。

> **二次评审修正（关键）**：watchdog 起初只加在 `tool_exec::execute_approved_tools`，但 ReadOnly 工具(正是这 7 个 fs-read)会在 LLM 流式阶段被 `streaming_tool_exec::feed_tool` **提前启动**，该路径**绕过** watchdog——等于早启动的 glob/grep 又回到"无外层超时"的原始挂死。修复：抽出单一收敛点 `execute_tool_watchdogged`，**两条执行路径(早启动 + 正常审批)都经它**，杜绝"一条路有界、另一条无界"。并补全 ReadPdf/ReadImage/ReadHtml(同为 ReadOnly、同样早启动)。

## 3. 实施的改动

| 改动 | 文件 | 对应发现 |
|---|---|---|
| 后端外层 `tokio::time::timeout` + `WALK_TIMEOUT_GRACE` **删除**，`*_search_async` 回归纯 `spawn_blocking` | `search/mod.rs` | #1 #5 #12 |
| `watchdog_deadline` 扩展覆盖 7 个 fs-read 工具（固定 `FS_READ_TIMEOUT=60s`） | `tool_watchdog.rs` | #1 |
| 抽 `execute_tool_watchdogged` 单一收敛点，早启动路径(`feed_tool`)与审批路径都经它 | `tool_exec.rs` `streaming_tool_exec.rs` | 二次评审(早启动绕过) |
| glob/grep `max` 钳到 `≥1`，消除 `max==0` 时 truncated 恒真 + 写空 overflow | `search/glob.rs` `search/grep.rs` | 二次评审(max==0) |
| `truncated` 改由"命中 cap"派生（glob `entries.len()>=max`；grep `total>=max`），与超时解耦 → 超时不再写空 overflow | `search/glob.rs` `search/grep.rs` | #3 |
| `SEARCH_TIMEOUT_NOTICE` 提到 `loopal-tool-api`，两个工具共享 | `tool-api/truncate.rs` 等 | #9 |
| glob 删除冗余 `count` 原子，cap 检查折进 push 锁 | `search/glob.rs` | #11 |
| `walk_timeout` doc 注释改准，不再夸大 dead-NFS | `limits.rs` | #4 |

## 4. 行为变化与未决

- **Ls/Read 新增 60s 看门狗**：健康本地盘上 Ls/Read 毫秒级完成，60s 极宽松；死挂载下从"永久挂起"变为"60s 后硬错误"。这是**有意的行为变化**，关闭了原始事故类。
- **`timed_out` 的 typed 信号**：硬超时现由 watchdog 提供 typed `StaleReason`；软协作式超时仍以文本提示面向 LLM（部分结果场景，文本是合适的指引而非控制信号）。
- 未做：follow_links 可配置开关（[0002](0002-parallel-glob-walker.md) §3.2，YAGNI）；`>max` 截断时并行结果集非确定（[0002](0002-parallel-glob-walker.md) §4.2，固有，已记录）。
