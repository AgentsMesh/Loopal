# Glob Traversal Hang — Parallel Walker & Symlink Policy (P1)

状态：已实现（仅 P1：glob 并行 walker + `follow_links(false)` + glob 排序补 `path` 二级键）。前置 P0 遍历超时见 [0001](0001-root-cause-analysis.md)。P2 旁路加固（`same_file_system`、读路径逃逸软上限）不在本方案范围。`GlobMatcher: Send + Sync` 已由编译期确认，§3.1 注的 clone 退化路径未触发。

| | |
|---|---|
| **需求** | 把 glob 的目录遍历从单线程改为并行（对齐 grep），并把共享 walker 的 `follow_links` 默认改为 `false` |
| **动机** | 单线程穷举是放大故障的因素之一；`follow_links(true)` 偏离 ripgrep 默认，是符号链接逃逸/环路的隐患 |
| **范围** | 仅 backend 搜索层（`crates/loopal-backend/src/search/`）+ glob 工具排序（`glob/src/lib.rs`）。不改工具入参、不改 IPC、不抽公共 framework |
| **非目标** | 不改 grep 行为（已并行）；不引入 `same_file_system`/路径逃逸上限（P2）；不加配置开关（YAGNI） |

---

## 1. 背景与目标

P0 已用「协作式 deadline + 外层硬超时」消除了"会话假死"这一**致命**问题（[0001](0001-root-cause-analysis.md) §5）。P1 处理 [0001](0001-root-cause-analysis.md) §3 暴露的两个**结构性弱点**，它们不致命但长期有害：

1. **glob 单线程穷举**（`search/glob.rs` 的 `for entry in walker.build().flatten()`）。grep 早已是 `build_parallel().run()`，glob 是仓库内唯一仍走单线程的 tree-walk。慢 IO 下单线程会被单个慢目录串行堵住，整体 wall-time = 所有目录耗时之和。
2. **`build_walker` 的 `follow_links(true)`**（`search/walker.rs`，glob/grep 共用）。跟随符号链接会跨设备、触发 iCloud 下载、并可能产生遍历环路；且偏离了 ripgrep（Grep 工具自我描述的底座）的默认 `false`。

**目标**

- glob 遍历并行化，wall-time 在大目录上随核数下降，且单个慢目录不再串行堵死全局。
- glob 与 grep 的遍历骨架在风格上对齐（同一套 `done`/`timed_out`/`deadline` 纪律），降低维护心智。
- `follow_links` 回到 `false`，与 ripgrep 默认一致，关闭符号链接逃逸/环路这一类隐患。
- 保持对外行为等价：相同 `pattern`/`path` 下的结果集合不变（除"符号链接目标不再被遍历"这一明确语义变更）。

**显式非目标**：见上表。特别地，**不**把 glob/grep 的并行骨架抽成共享 runner（理由见 §3.3）。

---

## 2. 现状基线（P0 之后）

| 组件 | 现状 |
|---|---|
| `search/glob.rs` | 单线程 `walker.build().flatten()`；收集 `Vec<GlobEntry>`；`entries.len() >= max` 时 `break`；P0 在循环顶部加了 `Instant::now() >= deadline` 协作式停止 |
| `search/grep.rs` | **已并行**：`w.build_parallel().run(\|\| visitor)`；`Arc<AtomicUsize>`(命中数)、`Arc<AtomicBool> done`(早停)、`Arc<AtomicBool> timed_out`(超时)、`Arc<Mutex<Vec<_>>>`(收集)；visitor 顶部检查 `done`/`deadline` |
| `search/walker.rs` | `WalkBuilder::new(path).follow_links(true)`，可选 type filter；**glob/grep 共用** |
| `glob/src/lib.rs` | 工具层对 backend 返回的 `entries` 按 `modified_secs` **降序排序**，再按 `offset/limit` 分页（backend 返回未排序） |
| 结果类型 | `GlobSearchResult { entries, truncated, timed_out, overflow_path }` |

> 关键：glob 的最终顺序由**工具层的 mtime 排序**决定，不依赖遍历顺序——这是并行化能保持兼容的前提（详见 §4.2）。

---

## 3. 设计与备选取舍

### 3.1 glob 并行化（对齐 grep 骨架）

把单线程循环替换为 `WalkParallel`，visitor 内完成"判文件 → 相对路径匹配 → 收集"。结构与 grep 一一对应：

```text
deadline = now + walk_timeout
count    : Arc<AtomicUsize>     // 已收集条数
done     : Arc<AtomicBool>      // 早停（命中 max 或超时）
timed_out: Arc<AtomicBool>      // 早停原因 = 超时
entries  : Arc<Mutex<Vec<GlobEntry>>>
matcher  : Arc<GlobMatcher>     // 见 3.1 注
search_path: Arc<PathBuf>

build_parallel().run(per-thread):
  visitor(entry):
    if done            -> Quit
    if now >= deadline -> done=1; timed_out=1; Quit          // 协作式超时（与 P0 同义）
    if !is_file        -> Continue
    rel = strip_prefix(search_path); if no rel -> Continue
    if !matcher.is_match(rel) -> Continue
    push GlobEntry{ path, modified_secs }
    if count.fetch_add(1)+1 >= max -> done=1; Quit            // 命中上限早停

entries = Arc::try_unwrap(entries)            // run() 已 join 所有线程
truncated = done.load()                        // 超时或上限都算截断（语义同 P0 单线程版）
timed_out = timed_out.load()
```

**3.1 注 — matcher 的跨线程共享**：`globset::GlobMatcher` 为 `Send + Sync`，故用 `Arc<GlobMatcher>` 跨线程共享只读匹配（无需每线程克隆）。若编译期发现某版本不满足 `Sync`，退化为每线程 `matcher.clone()`（`GlobMatcher: Clone`，内部 Arc 化，克隆廉价）——与 grep 对 `regex::Regex` 的 `.clone()` 做法同源。此判定在实现时由编译器兜底，不是运行期风险。

**3.1 注 — `Arc::try_unwrap` 的前提**：`WalkParallel::run()` 返回前会 join 全部 worker 线程，所有 visitor 闭包已析构，`entries` 只剩外层一个强引用，`try_unwrap().unwrap()` 必然成功。此前提与 grep 现有代码一致。

### 3.2 `follow_links` 策略：改为 `false`

`search/walker.rs` 的 `build_walker` 把 `follow_links(true)` 改为 `false`。

- **为什么是 `false`**：ripgrep 默认 `follow_links=false`（`-L` 才 opt-in）。Grep 工具描述自称"built on ripgrep"，当前 `true` 实际是**偏离**底座默认。改 `false` 是**回归**预期，不是新增限制。
- **可配置性决策**：**硬编码 `false`，不引入设置项**。理由：(a) 与 ripgrep 默认对齐后，"需要跟随符号链接搜索"是少数派需求；(b) YAGNI——在出现真实诉求前不加配置面（符合仓库 Principles）。若未来确有需要，再补 `search.follow_symlinks`（默认 false）的逃逸阀，届时是纯增量。
- **作用边界（重要，避免误读）**：本变更关闭的是"经由**符号链接**进入外部树/网络盘/产生环路"这一类。它**修不了** [0001](0001-root-cause-analysis.md) 的原始事故——那里的 `AppsMeterial` 是一个**真实目录**（NFS 挂载点），遍历是直接下钻、不涉及符号链接。原始 vector 由 P0（超时）兜底、由 P2（`same_file_system`）根除。三者**互补**，详见 §4.5。

### 3.3 备选方案对比

| 方案 | 描述 | 取舍 |
|---|---|---|
| **A（采纳）** | glob 内联并行化 + `follow_links(false)`，glob/grep 各自独立但骨架对齐 | 提速、消除环路隐患、与 grep 风格统一；改动可控 |
| B | 仅保留 P0 单线程+超时，不并行化 | 已能防假死，但慢 IO 下仍串行堵塞、wall-time 高；放着唯一单线程 walk 是技术债 |
| C | 抽 `parallel_walk_with_deadline(visitor)` 公共 runner，glob/grep 共用 | **暂不做**：仅 2 个消费者，且 visitor 主体（文件名匹配 vs 读文件正则）差异大，共享部分只有 ~15 行样板；过早抽象违背既有偏好（tool 优化按 tool 单独做、不造横切 framework）。**触发条件**：出现第 3 个并行搜索消费者时再抽取 |
| D | `follow_links` 加配置开关 | YAGNI，见 §3.2 |

---

## 4. 正确性与一致性分析

### 4.1 `max` 早停在并行下是"近似"的

单线程版精确在 `entries.len() == max` 处 `break`。并行版中，多个线程可能在 `done` 传播前各自完成一次 `push`，最终 `entries.len()` 可能**轻微越过** `max`（最坏约 `max + (线程数 - 1)`）。

- **是否可接受**：可接受。`max = max_glob_results = 10_000`，越过几十条对工具层分页（`DEFAULT_LIMIT=100`/用户 `limit`）无影响；`truncated=true` 照常置位、overflow 文件照常落盘。
- **与 grep 一致**：grep 的 `total_match_count` 早停同样是 `fetch_add` 后判断，本就允许轻微越界。glob 采用同一近似，是**一致性收敛**而非新引入的不确定。

### 4.2 结果顺序与确定性（本方案唯一需要主动补强的点）

并行遍历的**收集顺序非确定**（线程交错）。但 glob 的对外顺序由**工具层 mtime 降序排序**决定，因此：

- **不同 mtime 的文件**：顺序完全不受影响（由 mtime 决定）。
- **相同 mtime 的文件**（同一秒 checkout/解压极常见）：`sort_by` 稳定，但**输入顺序**变成非确定，于是平局项的相对顺序在多次运行间会**抖动**——当"同 mtime 文件数 > 分页 limit"时，首页落入哪些文件可能逐次不同。这是并行化引入的**真实（虽轻微）行为变化**。

**对策（纳入 P1）**：在 `glob/src/lib.rs` 的排序里加 `path` 作为**二级排序键**，把平局打破从"遍历顺序"改为"路径字典序"，恢复跨运行确定性：

```text
sort by (modified_secs desc, path asc)
```

成本一行、零风险，且让 glob 比 grep 更确定（grep 当前对 `file_matches` 不排序、本就非确定——见 §4.4 脚注）。

### 4.3 `truncated` / `timed_out` 语义保持

- `truncated = done.load()`：`done` 由"命中 max"或"超时"任一置位，与 P0 单线程版"两种情况都置 `truncated=true`"等价。
- `timed_out` 独立标记"截断原因=超时"，工具层据此输出"搜索超时、请缩小 path"提示（[0001](0001-root-cause-analysis.md) §5.4）。语义不变。
- overflow：`truncated` 为真时落盘，行为不变。

### 4.4 与 grep 的行为对齐

P1 后 glob 与 grep 共享同一套遍历纪律（`build_walker` → `build_parallel` → `done`/`timed_out`/`deadline` → `Arc<Mutex>` 收集），**对称且各自独立**。这降低维护心智，同时不引入共享抽象（§3.3-C）。

> 脚注：grep 目前对 `file_matches` **不做排序**，其文件顺序本就非确定。把 grep 也改成确定顺序属 P1 范围外的独立改进，本方案仅记录该观察，不在此处实施。

### 4.5 `follow_links=false` 与原始事故的关系（诚实说明）

| 逃逸 vector | 经由 | P0 超时 | P1 follow_links=false | P2 same_file_system |
|---|---|---|---|---|
| 直接下钻真实挂载目录（**原始事故** `AppsMeterial`） | 真实目录 | ✅ 兜底（限时返回） | ❌ 无关 | ✅ 根除（不跨设备） |
| 经符号链接进入外部树/网络盘 | symlink | ✅ 兜底 | ✅ 关闭 | ✅ 关闭 |
| 符号链接环路 | symlink | ✅ 兜底 | ✅ 关闭 | — |

结论：P1 的 `follow_links=false` 是**防御纵深**，覆盖与原始事故**相邻**的一类，与 P0/P2 互补；它**不**单独修复原始 vector，文档不夸大其作用。

---

## 5. 影响面 / 兼容 / 迁移

### 5.1 行为变化清单

1. **符号链接目标不再被遍历**（语义变更，glob+grep 同时生效）。仅经由符号链接可达的文件不再出现在结果中。
   - 风险评估：pnpm 等的 `node_modules` 符号链接通常被 `.gitignore`，walker 本就不进入，影响小；源码级 `src -> ../shared` 类符号链接较罕见但存在。
   - 缓解：与 ripgrep 默认一致，行为可预期；在 release note 注明。
2. **glob 平局顺序改为路径字典序**（§4.2）。对"同 mtime 文件数 > limit"的场景，首页内容更稳定——属**改善**，但与并行化前的具体首页可能不同。
3. **glob `truncated` 时 `entries` 数量可能轻微越过 `max`**（§4.1）。对外分页无感。

### 5.2 现有测试影响

- glob 现有用例（`glob_tool_test.rs`/`glob_tool_edge_test.rs`/`glob_type_filter_test.rs`）断言均为**集合包含**与**计数**（如 `contains("foo.rs")`、`Found 105 files. Showing 1-100`），不依赖具体平局顺序 → 预期继续通过。
- P0 超时用例（`search_timeout_test.rs` + glob 工具超时用例）：deadline 检查从"循环顶部"移到"visitor 顶部"，语义不变 → 预期继续通过。
- grep 全部用例：`build_walker` 改 `follow_links` 影响 grep，但现有 grep 用例不构造符号链接 → 预期不受影响。

### 5.3 兼容 / 迁移

- **无配置迁移**：不新增/不删除任何 settings 字段。
- **无工具入参变更**：`Glob`/`Grep` 的 JSON schema 不变，LLM 侧无感。
- **无 IPC/存储格式变更**。
- **release note**：注明"搜索默认不再跟随符号链接（与 ripgrep 对齐）"。

---

## 6. 测试与性能验收

### 6.1 测试计划（目标覆盖率 ≥95% 改动代码）

| 用例 | 验证点 | 备注 |
|---|---|---|
| 并行正确性 | 含多层嵌套子目录的树，`**/*.rs` 命中全部目标，与单线程结果**集合相等** | 顺序无关断言 |
| 符号链接不跟随 | 根内放一个指向外部文件/目录的 symlink，断言结果**不含**该目标 | `#[cfg(unix)]`，用 `std::os::unix::fs::symlink`；Windows 跳过 |
| 截断越界容忍 | `ResourceLimits{ max_glob_results: 10, .. }` + 50 个匹配文件，断言 `truncated==true` 且 `entries.len()` ∈ `[10, 10+N线程]` | 复用 P0 测试里的自定义 limits 手法 |
| 平局确定性 | 同时写入多个同 mtime 文件，**两次**调用结果顺序一致 | 验证 §4.2 二级 path 排序 |
| 超时仍生效 | `walk_timeout=0` 时 glob 返回 `timed_out=true` | P0 用例迁移确认不回归 |

### 6.2 性能验收

- **基准方法**：选一个大目录（如 loopal 仓库自身或合成 N=10⁵ 文件树），对"零命中模式"（最坏穷举）与"高命中模式"各跑单线程基线 vs 并行，测 wall-time。
- **验收标准**：
  - 大目录并行 wall-time **显著低于**单线程（数量级期望 ≈ `min(核数, ignore 默认线程数)` 倍加速，IO 受限时打折）。
  - 小目录（<100 文件）并行因线程启动开销**不出现可感知回归**（容差 < 1ms 级，用户无感）。
  - 无功能回归：并行结果集合 == 单线程结果集合。
- 不在文档中预填基准数字（尚未实测），仅固化方法与门槛。

### 6.3 风险登记

| 风险 | 等级 | 缓解 |
|---|---|---|
| `GlobMatcher` 非 `Sync` | 低 | 编译期暴露；退化为每线程 `clone()`（§3.1 注） |
| 平局顺序抖动 | 低 | §4.2 二级 path 排序 |
| `follow_links=false` 漏搜符号链接目标 | 中 | 与 ripgrep 默认一致；release note；必要时后续加逃逸阀 |
| `max` 轻微越界 | 低 | 与 grep 一致，分页无感（§4.1） |

---

## 7. 落地步骤（实现期参照，本方案不含源码改动）

1. `search/walker.rs`：`follow_links(true)` → `false`。
2. `search/glob.rs`：单线程循环 → `build_parallel().run(visitor)`，按 §3.1 骨架；引入 `parking_lot::Mutex`、`ignore::WalkState`、`Arc` 原子量。
3. `glob/src/lib.rs`：排序键改为 `(modified_secs desc, path asc)`（§4.2）。
4. 测试：按 §6.1 增/改用例；跑 `bazel test` 受影响 target + `--config=clippy` + `--config=rustfmt`。
5. 全量 `bazel build //...` 确认结构无回归。

---

## 8. 未决项

- **二级排序键是否默认开启**：建议默认开启（§4.2），零成本恢复确定性。若评审认为"对外顺序变化"也需避免，可讨论。
- **grep 文件顺序是否也确定化**：范围外（§4.4 脚注），可另开条目。
- **符号链接逃逸阀**：暂不做（§3.2），出现真实诉求再增量。
