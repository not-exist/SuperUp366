# SuperUp366

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
8. 错题本查看与一键清理
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
cargo test --workspace
pnpm --dir frontend build
cargo tauri build
```

## 声明

仅供协议研究与个人学习使用，请只对自己的账号使用。见 `NOTICE.md`。
