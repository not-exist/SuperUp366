# up366 TUI

一个基于 `up366` Python 客户端库的命令行 TUI（天学网学生端）。

本分支只包含：**从 `pyapi` 分支导入的 `up366` 库** + **一个交互式终端程序**。

## 已实现

| 功能 | 状态 |
|---|---|
| 账号口令登录、课程 / 作业浏览 | ✅ |
| 作业列表标记（单词 PK / 疑似跟读 / 已交 / 未交 / 已过期 / 已退回） | ✅ |
| 作业详情（`job/info` 原始 JSON） | ✅ |
| 试卷结构解析与题型统计 | ✅ |
| **单词 PK 提交**（输入分数与 total，随机抽词） | ✅ |
| **跟读作业 mp3 收集**（列出所需段数，逐段输入路径） | ✅（仅本地计划） |
| 跟读作业远端提交 | ⏸ 留空（提交报文格式待定） |
| 普通作业提交 | ⏸ 留空（按需求暂不实现） |
| 错题本概览（学科 / 统计 / 书籍 / 课程） | ✅（只读） |

## 安装

要求 Python >= 3.10，运行时依赖 `httpx>=0.27`。

```powershell
python -m pip install -e .
# 可选：安装 defusedxml，进一步加固试卷 XML 解析
python -m pip install -e ".[security]"
```

安装后可执行：

```powershell
up366-tui
# 或
python -m up366_tui
```

不想安装时：

```powershell
$env:PYTHONPATH = "src"
python -m up366_tui
```

## 登录

程序启动后交互输入账号与口令；口令使用 `getpass`，不会回显、不会写入磁盘。

也支持环境变量：

| 变量 | 用途 |
|---|---|
| `UP366_USER` | 账号 |
| `UP366_PASS` | 口令 |
| `UP366_CLIENT_ID` | 覆盖 `clientId`（可选；缺省使用本机指纹） |

也可用参数（注意命令行会留在 shell 历史里）：

```powershell
up366-tui --username <账号> --password <口令>
```

## 作业完成状态

课程作业列表接口 `/client/student/course/job` **不保证**每个作业都带完成标记。TUI 按官方 H5 作业页的做法合并状态：

1. 加载作业列表时并行请求 `/client/student/all/course/finish/job/v2`；
2. 用 `finishedList[].studyTaskId == jobId` 标记 `[已交]`；
3. `returnedList` 标记 `[已退回]`（退回重做）；
4. 选中作业后再用 `job/info` 的 `taskStatus` / `stuStatus` 校正（该字段是完成判据）；
5. 列表字段兜底规则与 H5 一致：`type == 1` 看 `extParam.taskStatus == 1`，`type == 3` 看 `extParam.stuStatus > 1`；
6. 未完成且已过 `endTime` 显示 `[已过期]`；无法判定的类型显示 `[状态?]`。

> 在作业列表中按 `r` 可强制刷新作业与完成状态。

## 单词 PK 提交

1. 主菜单选择 `1` 选择课程 / 作业，选中一个带 `[单词PK]` 标记的作业。
2. 主菜单选择 `2`。
3. 输入：
   - **分数**：本局答对的单词数；
   - **total**：本局总单词数。
4. 程序从该作业的 `pkBucketId` 词库中随机抽取 `total` 个单词，前 `score` 个标记为正确、其余标记为错误。
5. 预览选中的单词，确认后调用 `submit_word_pk()` 真实提交。

> 单词 PK 的客户端语义是 `score = rightWordList.length`，因此“分数”被解释为答对词数。
> 同一作业重复提交通常返回 `-13`（重复作答 / 次数用尽）；程序会提示并停止重试。

## 跟读作业

1. 选中一个非单词 PK 作业（主菜单 `1`）。
2. 主菜单选择 `3`：
   - 程序通过 `job/info` 取 `paperId`，再通过 `paper_detail` 读取试卷 XML；
   - 统计 `question_type = 9`（跟读）的题目数量；
   - 显示 **需要 N 段 mp3（每题一段）**，并列出每题的 `elementId`、分值、参考时长、句数、内容预览；
   - 逐段提示输入本地 `.mp3` 路径，校验存在性、文件类型与重复路径；
   - 生成并保存本地计划 JSON 到 `~/.up366-tui/readaloud-plans/`。
3. 若试卷结构无法读取，可退化为手动输入段数。

**提交功能留空**：当前版本不会上传音频，也不会向服务器提交成绩。

`readaloud.py` 中的 `submit_plan()` 是一个显式 stub，调用即抛出 `NotImplementedError`，便于后续确认报文格式后补齐。

## 只读功能

- **作业详情**：显示 `job/info` 原始 JSON、题型统计、跟读题明细。
- **错题本**：列出学科与错题/消灭统计，以及该学科下的书籍和课程。全部为只读接口。

## 测试

```powershell
python -m pytest -q
```

覆盖：

- `pyapi` 分支导入的 114 个库测试；
- TUI 核心语义：试卷 XML 解析（含跟读题、attachment、DTD 防护）、单词 PK 随机选词与边界校验、跟读计划生成与“提交留空”约束。

离线自检（不需要账号）：

```powershell
up366-tui --self-check
```

## 目录

```
src/up366/       从 pyapi 分支导入的 API 客户端
src/up366_tui/
  __main__.py    命令行入口、登录、self-check
  app.py         菜单、课程/作业、单词 PK、跟读、错题本
  paper.py       试卷 XML 解析与题型识别
  readaloud.py   跟读计划与显式留空的提交 stub
  wordpk.py      词库过滤、随机选词与预览
  ui.py          终端渲染、输入、表格（无第三方依赖）
tests/           库测试 + TUI 测试
```

## 边界与安全

- 单词 PK 是**真实的写操作**；请只对自己的账号使用。
- 跟读流程只写本地 JSON 计划，不上传音频、不提交成绩。
- 程序不保存口令；会话只存在于当前进程内。
- 解析服务端试卷 XML 时优先使用 `defusedxml`（若已安装），并在标准库解析前拒绝 `DOCTYPE` / `ENTITY` 与超大文档。
- 本项目是个人学习辅助工具，不保证与官方客户端的所有边界行为一致。
