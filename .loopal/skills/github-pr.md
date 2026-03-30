---
description: Commit, create PR, monitor CI, fix failures, merge
---

# GitHub PR 全流程

你需要完成从 commit 到 merge 的完整 PR 流程。严格按照以下阶段执行，每个阶段完成后报告进度。

用户补充说明：$ARGUMENTS

## Phase 1: Commit

1. 收集当前状态：
   - `git status`（查看变更文件）
   - `git diff HEAD`（查看完整 diff）
   - `git log -5 --oneline`（了解提交风格）
   - `git branch --show-current`（当前分支）

2. 分析所有变更，生成符合项目风格的 commit message：
   - 格式：`type: concise description`（如 `fix:`, `feat:`, `refactor:`）
   - 关注"为什么"而非"什么"
   - 检查是否有 `.env`、credentials 等敏感文件，如有则警告并排除

3. Stage 并 commit：
   - 优先 `git add` 具体文件，避免 `git add -A`
   - 使用 HEREDOC 格式提交
   - 如果 pre-commit hook 失败，修复问题后重新创建 **新** commit（不要 amend）

## Phase 2: Create PR

1. 如果在 main/master 分支上，先创建新分支：
   - 分支名：从 commit 内容推导，格式 `type/brief-description`
   - `git checkout -b <branch-name>`
   - 重新 cherry-pick 或 commit（如果需要）

2. 推送到远程：
   - `git push -u origin <branch-name>`

3. 创建 PR：
   ```
   gh pr create --title "<简洁标题>" --body "$(cat <<'EOF'
   ## Summary
   <1-3 bullet points>

   ## Changes
   <变更的文件和模块>

   ## Test plan
   - [ ] CI passes
   EOF
   )"
   ```

4. 记录 PR 编号，后续阶段会用到。

## Phase 3: Monitor CI

1. 等待 10 秒让 CI 启动，然后开始轮询：
   ```
   gh pr checks <PR#> --watch --fail-fast
   ```
   如果 `--watch` 不可用，则每 30 秒轮询一次 `gh pr checks <PR#>`。

2. 判断结果：
   - **全部通过** → 进入 Phase 5（Merge）
   - **有失败** → 进入 Phase 4（Fix）
   - **超过 15 分钟仍在 pending** → 报告状态，询问用户是否继续等待

## Phase 4: Fix CI Failures（循环）

1. 获取失败详情：
   - `gh pr checks <PR#>`（确认哪些 check 失败）
   - `gh run list --branch <branch> --limit 5`（找到失败的 run ID）
   - `gh run view <run-id> --log-failed`（获取失败日志）

2. 分析失败原因，常见类型：
   - 编译错误 → 修复代码
   - 测试失败 → 修复测试或实现
   - Clippy 警告 → 修复 lint 问题
   - 格式问题 → 运行 formatter

3. 修复代码（使用 Read/Edit/Write 工具）

4. 本地验证修复（根据失败类型选择）：
   - `bazel build //...`
   - `bazel build //... --config=clippy`
   - `bazel test //...`

5. 创建新 commit 并推送：
   - Commit message 格式：`fix: address CI failure - <具体描述>`
   - `git push`

6. 回到 Phase 3 重新监控。**最多重试 3 轮**，超过后报告剩余问题并停止。

## Phase 5: Merge

1. 最终确认所有 checks 通过：
   ```
   gh pr checks <PR#>
   ```

2. 合并 PR：
   ```
   gh pr merge <PR#> --squash --delete-branch
   ```
   使用 squash merge 保持主干历史干净。

3. 报告最终结果：
   - PR URL
   - 合并的 commit SHA
   - CI 修复轮次（如果有）

## 重要规则

- 每个阶段完成后简要汇报进度
- 遇到不确定的决策（如 CI 失败原因不明）时询问用户
- 绝不 force push
- 绝不直接 push 到 main/master
- 如果 PR 需要 review approval 才能 merge，报告并等待用户指示
