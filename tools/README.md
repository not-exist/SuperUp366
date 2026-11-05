# tools 索引

35 个研究/复现脚本 + 1 份真实报文夹具。全部为一次性工具，不参与 `pytest`（测试在 `pyapi` 分支）。

## 1. 容器与加密（7）

| 文件 | 用途 |
|---|---|
| `u3enc.js` | `.u3enc` 编解码：AES-128-CBC，前 16 字节 IV，PKCS#7 |
| `decrypt_tree.js` | 递归解密整棵资源树 |
| `explore_asar.js` | 读取改名后的 asar 容器（`app.u3a`）目录 |
| `dump_any_u3a.js` | 解任意 `.u3a` 归档 |
| `dump_archive.js` | 解 `client/resources` 下的归档 |
| `dump_all.js` | 批量导出安装目录中的全部 `.u3a` |
| `extract_sourcemaps.py` | 从 xotFullScreen source map 中提取 `sourcesContent` |

## 2. 二进制与字符串扫描（13）

| 文件 | 用途 |
|---|---|
| `scan_bin_strings.py` / `scan_bin2.py` | 二进制中按关键词扫描 strings |
| `scan_dir.py` | 按关键词扫描整个目录 |
| `sec_imports.py` / `sec_imports2.py` | 解析 PE 导入表，筛加密 / 网络 API |
| `sec_ctx.py` / `sec_binctx.py` | 关键词上下文提取（文本 / 二进制） |
| `sec_urls.py` | 从二进制中提取 URL |
| `pe_debug.py` / `pe_layout.py` | PE 节表与布局分析 |
| `grep_ctx.py` | 带上下文的正则 grep |
| `dump_str.py` | 按偏移 dump 字符串 |
| `find_feature.py` | 在整个安装树中定位某功能名的全部痕迹 |

## 3. 协议与接口复现（3）

| 文件 | 用途 |
|---|---|
| `get_words.py` | 拉取单词 PK 真实词表 |
| `probe_words.py` | 单词 PK 各端点探测 |
| `sample_submit.py` | 单词 PK 提交接口样例 |

## 4. 逐句跟读流水线（9）

| 文件 | 用途 |
|---|---|
| `readaloud_job.py` | 定位「Unit 4 课文1 逐句跟读」作业 id |
| `readaloud_prepare.py` | 导出题目清单 → `work/readaloud-prep/task.json` |
| `readaloud_build.py` | 构造成绩报文（只构造，不发送） |
| `readaloud_upload.py` | 音频 OSS 直传 |
| `readaloud_post.py` | 直接 POST `/client/task/score/submit/v2` |
| `readaloud_submit.py` | 上传 → 写 `TaskStuded` 行 → 让客户端回传 |
| `read_setupcfg.js` | 本地成绩库（SQLCipher）读写 |
| `task_dump.py` / `task_report.py` | 任务数据导出与报告生成 |

## 5. 手动校验脚本（3，不进 pytest）

`test_objectname.py`（objectName 生成规则）、`test_readaloud.py`（`ut` 构造）、`test_readaloud_all.py`（整条链路）。

## 6. 夹具

`fixtures/real-submit-payloads.json` — 真实提交报文样本，含真实课程 / 任务标识，仅供比对。

## 凭据

脚本不保存账号。需要登录的脚本从 `UP366_USER` / `UP366_PASS` 环境变量或命令行参数读取。
