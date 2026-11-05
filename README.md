# SuperUp366

天学网（up366）PC 学生端的协议研究档案与 Python 客户端。仓库用两个分支存放两类内容：

| 分支 | 内容 | 入口 |
|---|---|---|
| `main`（默认） | 研究档案：协议 / 加密 / 安全分析、复现脚本、运行时探针、历史计划 | `recon/FINAL.md` |
| `pyapi` | 可安装的 Python 客户端 `up366` 与 114 个测试 | 切换到 `pyapi` 看它的 `README.md` |

```bash
git checkout pyapi    # 注意：切换会把另一分支的工作区文件从磁盘移出，切回即恢复
```

## 目录（本分支）

| 路径 | 内容 |
|---|---|
| `recon/` | 23 篇研究结论：客户端结构、加密、接口目录、安全评估、实机验证 |
| `tools/` | 35 个复现脚本 + `fixtures/` 真实报文夹具，索引见 `tools/README.md` |
| `harness/` | 客户端注入与运行时探针（Electron / asar / IPC） |
| `docs/` | `library-api.md` 库 API 参考；`plans/` 历史实施计划 |

## 快速导航

- 总入口：`recon/FINAL.md`
- 接口目录：`recon/api.md`
- 安全评估：`recon/security.md`
- 库 API：`docs/library-api.md`（与 `pyapi` 分支的 `src/up366/README.md` 同源）
- 脚本索引：`tools/README.md`

## 不在仓库内

以下目录是可重建的本地草稿，已 gitignore：

| 路径 | 说明 | 重建 |
|---|---|---|
| `work/client/` | 官方安装目录的 robocopy 镜像 | `harness/dev-prepare.ps1` |
| `extract/` 等 | 解密后的客户端源码（`recon/` 按字节偏移引用） | `tools/decrypt_tree.js` 等 |
| `work/` 其余 | 抓包、临时产物 | 按 `recon/` 的 runbook 重跑 |

本分支的工作区里还会看到 `src/`、`tests/`、`pyproject.toml` —— 它们属于 `pyapi` 分支，在本分支被忽略。

## 声明

仅供协议研究与互操作性分析，见 `NOTICE.md`。仓库不含任何真实账号凭据。
