# AGENTS.md

本文件约定在本仓库中执行任务时必须遵守的规则。优先级高于个人偏好。

## 账号登录约定（硬性）

1. **一次任务只允许登录正式账号一次。**
   - 同一次任务中，如果需要真实账号数据，只登录一次；不要为了“再确认一下”重复登录。
   - 登录产生的 **TGT、cookie、clientId、userType** 必须留存下来，用于后续只读探索，而不是再次登录。

2. **留存位置**
   - 账户元数据：SQLite `accounts` 表（`%APPDATA%\superup366\up366.db`）。
   - 会话密钥（TGT / cookies / client_id / user_type）：系统凭据库，一条账户一条记录。
   - 两者都由 `up366-store` 管理，绝不要把明文口令写入磁盘或提交到仓库。

3. **后续探索复用会话**
   - 需要复现请求时，从凭据库读取 `Session`，再 `Client::with_session(session)` 直接调用端点；
     参考 `src-tauri/examples/probe_*.rs` 的写法。
   - 会话失效（`-21` / `-22` / `-23`）时，代码会标记 `expired`；此时**先报告**，不要再自行登录第二次。

4. **写操作**
   - 单词 PK、订正等写接口会消耗次数或产生不可撤销的服务器状态，默认只做预览/干跑。
   - 未获得明确授权时不要对真实账号执行写请求；登录一次不等于授权反复写。

## 构建与验证

```bash
cargo test --workspace
cargo check --workspace --all-targets
pnpm --dir frontend build          # 内含 tsc --noEmit
```

交付前必须真实编译，而不是只跑单测：

```bash
cargo build --release -p up366-app --features custom-protocol
# 产物：target/release/up366-app.exe
```

## 代码结构

- `up366-core`：纯领域模型，无 I/O。
- `up366-protocol`：端点表、编码、签名、传输、DTO 与适配器。
- `up366-store`：SQLite 与系统凭据库。
- `up366-sync`：聚合、缓存与流程编排。
- `src-tauri`：命令层（薄）；一个功能一个模块。
- `frontend`：React 界面，只通过 Tauri 命令访问网络。

规则：

1. 任何写命令都是 `*_preview`（或 `*_draw`）+ `*_submit(confirm_token)` 两步，令牌一次性。
2. 命令层不手工拼请求体；URL 只能来自冻结的端点表。
3. 协议字段名只出现在 `up366-protocol`；上层使用带 `raw` 的强类型模型。
4. 不保留无用的兼容层；删除的代码直接删干净。

## 文档

- `docs/protocol/confirmed.md`：已验证的协议事实。
- `docs/protocol/to-probe.md`：尚未验证、不得用于真实写作的分支。
- 修改命令签名时同步 `frontend/src/api/types.ts` 与 `docs/commands.md`。
