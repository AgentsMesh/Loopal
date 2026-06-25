# Glob Traversal Hang — Root Cause Analysis

状态：P0 遍历超时已实现。P1 glob 并行 walker + `follow_links(false)` 已实现，见 [0002](0002-parallel-glob-walker.md)。⚠️ 本文 §5.3 的"后端外层硬兜底"已被一次 code review **取代**——外层超时合并到 runtime 通用 watchdog（并覆盖 Ls/Read），详见 [0003](0003-post-review-consolidation.md)。P2 旁路加固未做。

| | |
|---|---|
| **故障会话** | `fa027124-a400-4a24-9299-3684945b83f2`（cwd `/Users/stone/Works/AgentsMesh/GTMApps`，model gpt-5.5）|
| **挂死时间** | `2026-06-24T09:36:55Z`（本地 17:36）|
| **现象** | 会话停止响应，无报错、无崩溃；agent loop 永久 `await` 一个不返回的工具结果 |
| **定性** | **不是崩溃**，是 Glob 工具在网络挂载点上做*无超时 + 单线程 + 0 命中*的穷举遍历，堵死 `spawn_blocking` 线程 |
| **触发器** | 单次 `Glob` 调用把 `path` 指到 `/Users/stone/Works`（覆盖了一个 rclone/OSS 网络挂载点）|

---

## 1. 摘要

一次 `Glob { path: "/Users/stone/Works", pattern: "**/*YoloShell*" }` 调用：

1. `path` 逃逸出项目目录，扩到整棵 `Works` 树；
2. `Works` 下挂着一个 rclone→阿里云 OSS 的 NFS 网络盘；
3. 模式 `**/*YoloShell*` **零命中**，于是遍历**永不触发提前退出**，被迫穷举整棵树（含网络盘）；
4. Glob 的遍历**单线程、无 deadline、`follow_links(true)`**，runtime 也**没有 per-tool 超时**。

四者叠加 → `spawn_blocking` 线程在网络盘上无限期遍历 → agent loop 永远拿不到结果 → 会话静默假死。

---

## 2. 故障现场（日志证据）

会话事件流 `turns.jsonl` 的**最后几行**，每个 `ToolBatch` 此前都在毫秒级收到 `StepUpdated → Done`，唯独最后一批没有：

```
422  09:35:35.211Z  StepUpdated  item_index=0 -> Done
423  09:35:35.214Z  StepUpdated  item_index=1 -> Done
424  09:35:35.220Z  StepUpdated  item_index=2 -> Done
...
431  09:36:55.618Z  StepAppended LlmCall  tools=['Glob','Grep']
432  09:36:55.625Z  StepAppended ToolBatch [ ('Glob','Pending'), ('Grep','Pending') ]
<EOF —— 此后再无任何 StepUpdated→Done>
```

第 431 行的 LLM 文本与调用参数：

```jsonc
// LLM 自述：
"我缺 YoloShell 的 App Store 链接，先从本地项目/素材里查。"

// 卡死的调用：
Glob { "path": "/Users/stone/Works", "pattern": "**/*YoloShell*", "limit": 80 }
Grep { "path": "/Users/stone/Works", "glob": "**/*.{json,md,txt,js,swift,plist}", ... }
```

网络挂载证据（`mount` 输出 + 挂载脚本）：

```
localhost:/AppMaterials on /Users/stone/Works/AppsMeterial (nfs, nodev, nosuid, mounted by stone)
```
```bash
# mount-appsmeterial.sh —— 把阿里云 OSS bucket 挂成 NFS
rclone nfsmount appmaterials:appmeterials "$HOME/Works/AppsMeterial" \
  --vfs-cache-mode full --vfs-cache-max-size 50G ...
```

> 对照：同一会话**更早**的 Glob `path=".../GTMApps"`（限定在项目内）秒回；唯独这次 `path="/Users/stone/Works"`（覆盖 OSS 挂载）挂死。差别只在"遍历范围是否覆盖到网络盘"。

---

## 3. 调用链与根因代码

### 调用链

```
LLM tool_use(Glob)
  └─ GlobTool::execute          crates/tools/filesystem/glob/src/lib.rs
       └─ ctx.backend.glob()    crates/loopal-backend/src/local_backend_impl.rs:57
            └─ glob_search_async crates/loopal-backend/src/search/mod.rs   ← spawn_blocking，无 timeout
                 └─ glob_search   crates/loopal-backend/src/search/glob.rs ← 单线程穷举，0 命中不退出
                      └─ build_walker  crates/.../search/walker.rs         ← follow_links(true)
```

### 因素 ①：`path` 逃逸出项目，且绝对读路径不做包含检查

`crates/tools/filesystem/glob/src/lib.rs`：

```rust
let search_path = match input.path.as_deref() {
    // resolve_path 的第二个参数 false = 不做 cwd 包含检查（读路径直通）
    Some(p) => Some(ctx.backend.resolve_path(p, false).map_err(/* ... */)?),
    None => None,
};
// ...
let result = ctx.backend.glob(&opts).await /* ← 整个 agent loop 在这里永久挂起 */ ?;
```

会话 cwd 是 `.../GTMApps`，但 LLM 传入的 `path` 是父目录 `/Users/stone/Works`。读路径不做 cwd 包含检查，遍历范围合法地扩大到整棵 `Works` 树——其中包含 `AppsMeterial` 这个 OSS 网络挂载点。

### 因素 ②③：单线程穷举遍历 + 0 命中永不提前退出

`crates/loopal-backend/src/search/glob.rs`（修复前）：

```rust
let max = opts.max_results.min(limits.max_glob_results);   // = 10_000
// ...
for entry in walker.build().flatten() {          // ← 单线程迭代器（对比 grep 用 build_parallel）
    if !entry.file_type().is_some_and(|ft| ft.is_file()) { continue; }
    let rel = match entry.path().strip_prefix(&search_path) { Ok(r) => r, Err(_) => continue };
    if !matcher.is_match(rel) { continue; }
    // ...
    entries.push(GlobEntry { /* ... */ });
    if entries.len() >= max { break; }           // ← 唯一的提前退出：命中数达到 10_000
}
```

**关键缺陷**：唯一的提前退出条件是"命中数达到 `max`"。而 `**/*YoloShell*` 在整棵 `Works` 树里**零命中**，`entries.len()` 恒为 0，永远 `break` 不了 → walker 必须**穷举遍历完每一个文件**才能返回空结果。这个穷举过程要钻进 OSS 网络盘，每次 `readdir`/`metadata` 都是网络往返。

### 因素 ④a：遍历跟随符号链接

`crates/loopal-backend/src/search/walker.rs`：

```rust
let mut builder = WalkBuilder::new(search_path);
builder.follow_links(true);   // ← 跟随符号链接，跨网络盘/iCloud 时可触发下载、跨设备、环路
```

> 注：`ignore` 的 `.gitignore` 过滤在此**不生效**——`/Users/stone/Works` 本身不是 git 仓库，gitignore 规则只在各子仓库内部应用，挡不住对网络盘的下钻。

### 因素 ④b：遍历层无超时，runtime 也无 per-tool 超时

`crates/loopal-backend/src/search/mod.rs`（修复前）：

```rust
pub async fn glob_search_async(opts, cwd, limits) -> Result<GlobSearchResult, ToolIoError> {
    tokio::task::spawn_blocking(move || glob_search(&opts, &cwd, &limits))  // ← 无 timeout 包裹
        .await
        .map_err(|e| ToolIoError::Other(e.to_string()))?
}
```

`crates/loopal-backend/src/limits.rs` 的 `default_timeout`（300s）只作用于 shell `exec`，glob/grep 遍历不受其约束；runtime 工具管线对 tool execute 也**没有任何 per-tool 超时**（仓库内仅有 LLM 重试退避与 compaction 的超时）。因此阻塞线程卡在网络盘 syscall 上时，**没有任何机制能打断它**，agent loop 只能无限期 `await`。

### 对照组：grep 为何相对不易挂（但同样有隐患）

`crates/loopal-backend/src/search/grep.rs` 用的是**并行 walker**：

```rust
w.build_parallel().run(|| { /* 每线程 visitor */ });
```

并在 `search_one_file` 命中 `max_grep_matches`（500）时设 `done`，visitor 顶部 `if done.load() { return WalkState::Quit; }` 早停。grep 多线程 + 命中上限更低，"卡死"概率比 glob 小。但 grep **修复前同样没有遍历超时**，在零命中 + 网络盘场景下一样会长时间穷举——只是这次 ToolBatch 是两者并行、整批等最慢的 Glob，所以表面看是 Glob 挂死。

---

## 4. 影响面

- **严重度**：高。任何一次把 `path` 指向包含慢 IO（OSS/iCloud/坏 NFS/超大目录）的目录的 Glob/Grep，都能让**整个会话静默假死**，且日志只留下一个 `Pending` 的 ToolBatch，排查成本极高。
- **不限于网络盘**：`/Users/stone/Works` 级别的超大本地目录树 + 零命中模式，也会造成数十秒到数分钟级的卡顿。
- **后台线程泄漏**：外层 `tokio::time::timeout` 只能让 agent loop 不再 `await`；`spawn_blocking` 的阻塞线程**不会被 tokio 取消**，会继续在网络盘上空跑——因此还需要遍历内部的协作式停止。

---

## 5. 已实施的修复（P0 遍历超时）

主方案为"遍历超时"，采用**双层**设计，缺一不可：协作式 deadline 负责"停掉后台线程并带回部分结果"，外层 `timeout` 负责"无论如何解放 agent loop"。

### 5.1 `ResourceLimits` 新增 `walk_timeout`

`crates/loopal-backend/src/limits.rs`：默认 `Duration::from_secs(30)`。所有既有 `ResourceLimits { .. }` 字面量都用 `..Default::default()`，故无需改动任何调用点。

### 5.2 遍历内协作式 deadline

`glob.rs`（单线程循环顶部）：

```rust
let deadline = Instant::now() + limits.walk_timeout;
// ...
for entry in walker.build().flatten() {
    if Instant::now() >= deadline {
        timed_out = true;
        truncated = true;
        break;                      // 带着已收集的部分结果返回
    }
    // ...
}
```

`grep.rs`（并行 visitor 顶部，复用已有 `done` 早停旗标）：

```rust
let deadline = Instant::now() + limits.walk_timeout;
// ...
Box::new(move |entry| {
    if done.load(Ordering::Relaxed) { return WalkState::Quit; }
    if Instant::now() >= deadline {
        done.store(true, Ordering::Relaxed);
        timed_out.store(true, Ordering::Relaxed);
        return WalkState::Quit;
    }
    // ...
})
```

> 协作式检查只在两次 entry 之间生效，无法打断正卡在单个 `readdir`/`metadata` syscall 里的线程——那种极端情况由 5.3 的外层硬兜底兜住。

### 5.3 外层硬兜底 `tokio::time::timeout`

`crates/loopal-backend/src/search/mod.rs`：

```rust
const WALK_TIMEOUT_GRACE: Duration = Duration::from_secs(10);

pub async fn glob_search_async(opts, cwd, limits) -> Result<GlobSearchResult, ToolIoError> {
    let hard = limits.walk_timeout.saturating_add(WALK_TIMEOUT_GRACE);
    let join = tokio::task::spawn_blocking(move || glob_search(&opts, &cwd, &limits));
    match tokio::time::timeout(hard, join).await {
        Ok(joined) => joined.map_err(|e| ToolIoError::Other(e.to_string()))?,
        Err(_) => Err(ToolIoError::Timeout(hard)),   // ← agent loop 一定能在 hard 内解放
    }
}
```

正常情况下协作式 deadline（30s）先返回部分结果，外层（40s）只在内层卡死 syscall 时才触发并返回 `Timeout`。`grep_search_async` 同构。

### 5.4 向 LLM 暴露超时信号

`GlobSearchResult` / `GrepSearchResult` 各加 `timed_out: bool`。工具层据此输出明确提示（`glob/src/lib.rs`、`grep/src/grep_format.rs`），让 LLM 知道"结果不完整、请缩小 `path`"，避免对一个超时返回的空结果误判为"确实没有"：

```
⚠️ Search timed out before scanning the whole tree — results are incomplete.
   Narrow `path` to a specific project subdirectory and retry ...
```

### 5.5 测试

- `crates/loopal-backend/tests/suite/search_timeout_test.rs`：`walk_timeout=0` 时 `glob_search`/`grep_search`/`*_async` 均返回 `timed_out=true`；默认预算时正常完成且命中正确。
- `glob_tool_edge_test.rs`：零预算 backend 下工具输出含 "timed out"。
- `grep_timeout_test.rs`：直接单测 `format_results` 的超时提示（空结果替换、有结果追加、未超时不追加）。

全量 `bazel build //...`、受影响 `bazel test`、`--config=clippy`、`--config=rustfmt` 均通过。

---

## 6. 未做项（待后续排期）

- **P1 — glob 改并行 walker + `follow_links(false)`**：已按独立方案 [0002](0002-parallel-glob-walker.md) 实现。提速且消除符号链接环路，但非"防挂死"必需。
- **P2 — 旁路加固**：walker 加 `same_file_system(true)` 不跨越文件系统边界进入 NFS/OSS 挂载；对读路径做"逃逸出 cwd 过多层"的软上限/提示。

---

## 附：涉及文件清单

| 文件 | 角色 / 改动 |
|---|---|
| `crates/loopal-tool-api/src/backend_types.rs` | `GlobSearchResult`/`GrepSearchResult` 加 `timed_out` |
| `crates/loopal-backend/src/limits.rs` | `ResourceLimits` 加 `walk_timeout`（默认 30s）|
| `crates/loopal-backend/src/search/glob.rs` | 单线程循环内协作式 deadline；构造点补 `timed_out` |
| `crates/loopal-backend/src/search/grep.rs` | 并行 visitor 内协作式 deadline；构造点补 `timed_out` |
| `crates/loopal-backend/src/search/grep_file.rs` | `empty_result`/`search_single_file` 构造点补 `timed_out` |
| `crates/loopal-backend/src/search/mod.rs` | `*_search_async` 外层 `tokio::time::timeout` 硬兜底 |
| `crates/tools/filesystem/glob/src/lib.rs` | `timed_out` 输出提示 |
| `crates/tools/filesystem/grep/src/grep_format.rs` | `timed_out` 输出提示 |
| `crates/loopal-backend/src/search/walker.rs` | 现状 `follow_links(true)`（P1 待改）|
