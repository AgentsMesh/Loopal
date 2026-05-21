# Stage 1: Vault on Hub

实施 ADR A1/B1/C2/D2/E1/F2。范围：Vault 从 agent 进程搬到 Hub，Provider api_key 改 plaintext-on-demand，sub-agent 加 cwd 祖先链校验。

## 1. IPC 协议（新增 `hub/secret_*`）

JSON-RPC 2.0 over 现有 hub IPC transport。

```
method: hub/secret_get
params: {
  cwd: string,                     // canonical absolute path
  name: string,                    // [a-z][a-z0-9_]*
  caller: { agent_name, depth, tool_name? }
}
result: { plaintext: string }     // agent 收到后立即包 Zeroizing<String>
errors:
  -32001 VaultNotFound             // cwd 下无 .loopal/vaults/
  -32002 SecretNotFound            // vault 内无此 name
  -32003 PermissionDenied          // caller 不在 cwd 祖先链上
  -32004 DecryptFailed             // age 解密失败

method: hub/secret_list_names
params: { cwd: string }
result: { names: string[] }       // 仅名字，无 plaintext

method: hub/secret_health
params: { cwd: string }
result: { vault_count, default_vault, last_op_ts }
```

`caller` 字段用于 audit，**不**用于权限决策。权限由 Hub 自己维护的 spawn registry 决定（见 §3）。

## 2. Agent 端：SecretClient trait + 适配器

新 crate `loopal-secret-client`：

```rust
#[async_trait]
pub trait SecretClient: Send + Sync {
    async fn get(&self, name: &str) -> Result<Zeroizing<String>, SecretError>;
    async fn list_names(&self) -> Result<Vec<String>, SecretError>;
    async fn expand_author(&self, template: &str) -> Result<Zeroizing<String>, SecretError>;
    async fn expand_wire(&self, template: &str, allowed: &[&str])
        -> Result<Zeroizing<String>, SecretError>;
}

pub struct HubSecretClient {
    connection: Arc<Connection>,        // 到 Hub 的 IPC
    cwd: PathBuf,                       // canonical
    agent_name: String,
    depth: u32,
}

pub enum SecretError {
    VaultNotFound,
    SecretNotFound(String),
    PermissionDenied,
    DecryptFailed,
    Ipc(IpcError),
}
```

`expand_author` / `expand_wire` 实现：

1. 用正则扫 `template` 找占位符（`{{secret:X}}` 或 `<secret_ref:X>`）
2. 对每个 NAME 调 `self.get(NAME)` 拿 `Zeroizing<String>`
3. 中间 buffer 用 `Zeroizing<String>` 累积
4. 返回最终 `Zeroizing<String>`，drop 时整段 zero

`expand_wire` 多一个 `allowed: &[&str]` —— 只对 allowed 字段名做替换，其他位置保留 `<secret_ref:X>`。

## 3. Hub 端：HubVaultService + SpawnRegistry

新 crate `loopal-hub-vault`：

```rust
pub struct HubVaultService {
    vaults: tokio::sync::RwLock<HashMap<PathBuf, Arc<AgeVault>>>,
    audit: Arc<JsonlAuditSink>,
    spawn_registry: Arc<SpawnRegistry>,   // 由 Hub 注入
}

impl HubVaultService {
    pub async fn handle_secret_get(
        &self,
        caller_agent_id: &str,           // from IPC connection identity
        req: SecretGetRequest,
    ) -> Result<SecretGetResponse, SecretError>;
}
```

实现步骤：

1. canonicalize `req.cwd`
2. `spawn_registry.verify_vault_access(caller_agent_id, &req.cwd)` —— 见 §4
3. lazy 加载 vault：`vaults.entry(cwd).or_insert_with(|| AgeVault::open(...))`
4. `vault.get(&req.name).await` 拿 plaintext
5. 写一条 audit 记录到 `~/.loopal/telemetry/secret_access.jsonl`
6. 返回 plaintext（IPC response 序列化时是普通 String，agent 收到后立即包 Zeroizing）

vault 实例的生命周期：lazy 加载，永不驱逐（Hub 重启时全部丢弃 = O1 stateless）。

## 4. SpawnRegistry：父子关系 + cwd 祖先链

`loopal-agent-hub/src/spawn_registry.rs`（新增）：

```rust
pub struct SpawnRegistry {
    entries: RwLock<HashMap<AgentId, SpawnEntry>>,
}

struct SpawnEntry {
    cwd: PathBuf,                    // canonical
    parent_id: Option<AgentId>,
}

impl SpawnRegistry {
    pub fn register(&self, id: AgentId, cwd: PathBuf, parent_id: Option<AgentId>);
    pub fn unregister(&self, id: &AgentId);
    
    pub fn verify_vault_access(&self, caller_id: &AgentId, target_cwd: &Path) -> bool {
        // 收集 caller 的 spawn root：沿 parent_id 链找到最顶层（parent_id = None）
        // 该 root 的 cwd 即"祖先 cwd"
        // target_cwd 必须是 root_cwd 的祖先或后裔（含相等）
        // 即 root_cwd.starts_with(target_cwd) || target_cwd.starts_with(root_cwd)
    }
}
```

注册时机：`spawn_and_register` 完成时调 `register`；agent disconnect 时调 `unregister`。

## 5. Provider 改造（plaintext-on-demand）

`loopal-provider-api/src/lib.rs`：

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    // 既有方法...
    
    fn set_secret_client(&mut self, client: Arc<dyn SecretClient>);
}
```

每个 Provider impl（Anthropic / OpenAI / Google / OpenAI-compat）：

```rust
pub struct AnthropicProvider {
    base_url: String,
    api_key_ref: String,                      // 占位符字面量，例如 "{{secret:anthropic_key}}"
    secret_client: Option<Arc<dyn SecretClient>>,
    http: reqwest::Client,                    // 不持有 api_key
}

impl AnthropicProvider {
    async fn auth_header(&self) -> Result<Zeroizing<String>> {
        match &self.secret_client {
            Some(c) => c.expand_author(&self.api_key_ref).await
                .map_err(|e| ProviderError::SecretExpand(e)),
            None => Ok(Zeroizing::new(self.api_key_ref.clone())),  // test mode
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn send(&self, req: ChatRequest) -> Result<ChatResponse> {
        let auth = self.auth_header().await?;             // 拉 plaintext
        let resp = self.http.post(&self.url)
            .header("x-api-key", auth.as_str())           // 借引用，不 clone
            .json(&req)
            .send()
            .await?;
        // auth 在此 scope 末尾 drop → memory zeroed
        process_response(resp).await
    }
}
```

测试模式（`secret_client = None`）下 `api_key_ref` 直接当 plaintext，方便单元测试。

## 6. Kernel 字段变更

`loopal-kernel/src/kernel/mod.rs`：

```rust
pub struct Kernel {
    pub(super) tool_registry: ToolRegistry,
    provider_registry: ProviderRegistry,
    hook_service: HookService,
    pub(super) mcp_manager: Arc<RwLock<McpManager>>,    // Stage 2 才搬走
    // ... 其他保留
    
    // 新增
    secret_client: Option<Arc<dyn SecretClient>>,
    
    // 删除
    // secrets: Option<Arc<dyn Vault>>,  ← 删
}
```

`build_kernel_from_config` 流程改造：
1. 不再调 `build_secret_store`
2. 构造 `HubSecretClient(connection, cwd, agent_name, depth)`
3. `kernel.set_secret_client(Arc::new(client))`
4. 遍历 provider_registry，对每个 provider 调 `set_secret_client(client.clone())`

## 7. CLI 路径不动

`loopal vault` / `loopal vaults` 子命令继续直接调 `AgeVault` API，不经 Hub。**A1 决策的体现**。这意味着：

- 管理操作（set/get/list/rekey/recipients）不需要 Hub 运行
- vault 文件是 ground truth
- 运行时操作（agent 请求 plaintext）必须走 Hub

## 8. PR 拆分序列

每个 PR 独立可 compile + test：

1. **PR-1**：新 crate `loopal-secret-client`（trait + error 类型，无依赖）
2. **PR-2**：新 crate `loopal-hub-vault`（vault 服务实现，不挂 IPC）
3. **PR-3**：`SpawnRegistry` 加进 `loopal-agent-hub`，spawn_and_register 时记录父子关系；提供 `verify_vault_access`
4. **PR-4**：把 `HubVaultService` IPC handler 接到 `loopal-agent-hub` 的 dispatch 表，处理 `hub/secret_*`
5. **PR-5**：实现 `HubSecretClient`（IPC 客户端），有 mock IPC 单元测试
6. **PR-6**：`Kernel` 字段切换 + `build_kernel_from_config` 改造 + Provider trait 加 `set_secret_client`
7. **PR-7**：Anthropic Provider 改造 + 集成测试
8. **PR-8**：OpenAI / Google / OpenAI-compat Provider 改造
9. **PR-9**：`loopal-secret-runtime` 内 `apply_resolver` / `apply_redactor` 改成走 `SecretClient`
10. **PR-10**：删除 agent 路径上对 `build_secret_store` 的引用 + 旧 `Arc<dyn Vault>` 字段
11. **PR-11**：端到端 smoke test（实际 vault + 实际 Hub IPC）

PR-1/2/3/4 可以并行；PR-5 依赖 PR-1/4；PR-6 依赖 PR-5；PR-7/8 依赖 PR-6；PR-9 依赖 PR-5；PR-10 依赖前面全部。

## 9. 测试策略

- **单元测试**：每个 crate 内的纯逻辑（SecretError 转换、cwd 祖先链算法、模板解析正则）
- **Mock IPC 测试**：在 `loopal-secret-client` 用 mock Connection 驱动 HubSecretClient，验证 IPC 请求字段正确、错误映射正确
- **HubVaultService 隔离测试**：用 tmpdir vault + JsonlAuditSink 测真实 age 解密
- **集成测试**：`loopal-agent-server/tests/suite/secret_via_hub_test.rs`，启动真实 Hub + 真实 agent server，验证一次 LLM 调用的 auth header 走完整路径
- **Smoke test**：macmini-03-64 上跑一个完整 turn，含 Bash secret expand + LLM auth

## 10. 验收标准

- `git grep "Arc<dyn Vault>" crates/loopal-{runtime,agent,kernel,provider}/` 返回 0
- `git grep "build_secret_store" src/ crates/loopal-{runtime,agent,kernel,provider}/` 返回 0
- `~/.loopal/telemetry/secret_access.jsonl` 写入方 PID 是 Hub 进程
- `cargo test --workspace` 全绿（含 sentinel tracing test 不被 plaintext 污染）
- Smoke: 一个 LLM turn 完成，期间触发至少一次 `hub/secret_get`
- `cargo build` 通过且 `Zeroizing<String>` 的 clippy lint 无 warning
