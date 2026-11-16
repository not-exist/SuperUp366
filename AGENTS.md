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
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
pnpm --dir frontend lint
pnpm --dir frontend typecheck
pnpm --dir frontend e2e            # build:e2e + Playwright；全部命令走 mock，不联网
cargo deny check                   # 依赖审计（需预先安装 cargo-deny；CI 用同一份 deny.toml）
```

`pnpm --dir frontend e2e` 使用 `src/api/mock.ts` 作为假后端，不需要账号，也不会发出网络请求；
新增页面应同时补 `frontend/e2e/*.spec.ts` 与稳定的 `data-testid`。

交付前必须真实编译，而不是只跑单测：

```bash
cargo build --release -p up366-app --features custom-protocol
# 产物：target/release/up366-app.exe（Windows）或 up366-app（Linux）

# 安装包（@tauri-apps/cli 在 frontend devDependencies 中）
node frontend/node_modules/@tauri-apps/cli/tauri.js build --bundles nsis          # Windows
node frontend/node_modules/@tauri-apps/cli/tauri.js build --bundles deb,appimage  # Linux
# 产物：target/release/bundle/
```

## CI

`.github/workflows/` 下的五个文件：

| 文件 | 作用 |
|---|---|
| `ci.yml` | 主门禁：quality → test（Windows + Ubuntu）/ e2e / audit → package |
| `package.yml` | 可复用：双平台编译并打包 nsis / deb+appimage，上传为 action 产物 |
| `audit.yml` | 可复用 + 每周定时：cargo-deny（advisories/bans/sources/licenses）与 pnpm audit |
| `release.yml` | `v*` tag：复用 package，创建 draft release |
| `codeql.yml` | CodeQL advanced setup：rust / javascript-typescript / actions，push、PR 与每周定时上传 SARIF |

- PR 也执行打包，产物挂在 run 的 Artifacts 区；所有检查都不需要 Secret。
- 新增依赖后若 `cargo-deny` 报许可证，先确认许可证再改 `deny.toml` 的 allow 列表。
- CI 与本地使用同一套命令；失败时优先在本地复现对应 job 的步骤。

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
