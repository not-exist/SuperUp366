# SuperUp366

[![CI](https://github.com/not-exist/SuperUp366/actions/workflows/ci.yml/badge.svg?branch=dev)](https://github.com/not-exist/SuperUp366/actions/workflows/ci.yml)

天学网（up366）学生端的本地自用桌面客户端。

- **Rust** — 协议、领域模型、缓存与安全存储
- **React + TypeScript + Vite** — 界面
- **Tauri 2** — 桌面壳

## 分支

| 分支 | 内容 |
|---|---|
| `main` | 协议逆向研究档案（`recon/`、`tools/`、`harness/`） |
| `pyapi` | 早期 Python 客户端实现（仅作对照，不再维护） |
| `dev` | 本实现（orphan 分支，与上述历史无共同祖先） |

`dev` 是独立 orphan 分支：旧 Python 代码不参与构建，只在需要核对协议时参考 `main` 的 `recon/`。

## 功能范围

1. 登录与多账号切换（保存会话状态，不保存密码）
2. 跨班级合并的作业列表（保持官方顺序，标注所属班级）
3. 书籍选择与目录
4. 普通作业完成状态与分数
5. 普通作业逐题答案
6. 单词 PK
7. 朗读跟读 mp3 上传与提交（作业 + 书籍）
8. 错题本一键订正（抽题预览 → 标准答案提交 → 批改状态查询）
9. 性能与简洁 UI

## 目录

```text
crates/
  up366-core/      纯领域模型（无 I/O）
  up366-protocol/  HTTP、认证、端点表、编码与签名
  up366-store/     SQLite + 系统凭据库
  up366-media/     mp3 校验与 OSS 上传
  up366-sync/      聚合、缓存、并发、流程编排
src-tauri/         Tauri 应用壳（命令、状态、事件）
frontend/          React 界面
tests/fixtures/    脱敏协议报文
docs/protocol/     已确认协议与待探测项
```

## 开发

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked

pnpm --dir frontend lint
pnpm --dir frontend typecheck
pnpm --dir frontend e2e            # Playwright，全部命令由 mock 后端应答，不联网

# 独立可执行文件（内嵌前端，不依赖 localhost）
cargo build --release -p up366-app --features custom-protocol
# 产物：target/release/up366-app.exe

# 安装包：Windows 为 nsis，Linux 为 deb + AppImage
node frontend/node_modules/@tauri-apps/cli/tauri.js build --bundles nsis
node frontend/node_modules/@tauri-apps/cli/tauri.js build --bundles deb,appimage
# 产物：target/release/bundle/

# 或者调试时先起 Vite，再 cargo run（不带 custom-protocol 时读 devUrl）
pnpm --dir frontend dev
cargo run -p up366-app
```

CI（`.github/workflows/ci.yml`）在 Windows 与 Ubuntu 上跑测试，并执行 fmt、clippy、lint、
typecheck、Playwright e2e、cargo-deny / pnpm audit；全部通过后会把 Windows 安装包与
Ubuntu 的 `.deb` / `.AppImage` 上传到该次 action 的 Artifacts。

`custom-protocol` 特性决定 Tauri 加载内嵌的 `frontend/dist`；不带它时构建会去连
`build.devUrl`（默认 `http://localhost:5173`），没有开发服务器就会报
`ERR_CONNECTION_REFUSED`。

## 声明

仅供协议研究与个人学习使用，请只对自己的账号使用。见 `NOTICE.md`。
