# up366

最小化 up366（天学网）PC 学生端 API 客户端，Python 实现。基于官方客户端的登录态与请求协议，提供登录、课程 / 作业查询、教材（书架 / 目录）、普通作业、错题本查询以及**单词 PK 成绩提交**能力。

> 只描述代码里**真实存在**的公开 API。以下划线开头的方法为内部实现，**非公开 API**，本文不作 API 介绍。标「本批新增 / 实验性 / 未验证」者按标注理解，勿凭名称臆测其已在线上确认。

本轮（catalog rewrite）把端点收拢到 `endpoints.py`（冻结 `Endpoint` dataclass + `ENDPOINTS` 字典，32 条），并新增课本 / 普通作业 / 错题本三组方法，来自 `book.py` / `job_api.py` / `wrongnote.py` 三个 mixin（`class Up366(BookAPI, WrongNoteAPI, JobAPIMixin)`）。

## 1. 安装与导入

**src 布局**，包位于 `src/up366/`。模块：`client.py`（`login` / `Up366` / 会话）、`endpoints.py`（端点目录）、`form.py`（`form_encode`）、`words.py`（`build_word_infos`）、`errors.py`（异常）、`job.py`（`Job` / `JobStatus`）、`book.py` / `wrongnote.py`（模型 + mixin）、`job_api.py`（`JobAPIMixin`）、`__init__.py`（导出）。`pyproject.toml` 关键项：

```toml
requires-python = ">=3.10"
dependencies = ["httpx>=0.27"]
[tool.setuptools.packages.find]
where = ["src"]
[tool.pytest.ini_options]
pythonpath = ["src"]
testpaths = ["tests"]
```

```bash
pip install -e .                                # 方式 A：可编辑安装（推荐）
PYTHONPATH=src python your_script.py            # 方式 B：Linux/macOS
set PYTHONPATH=src && python your_script.py     # 方式 B：Windows cmd
$env:PYTHONPATH="src"; python your_script.py    # 方式 B：PowerShell
```

```python
from up366 import login, Up366, Job, JobStatus, Book, Chapter
from up366 import (WrongSubject, WrongSubjectStat, WrongBook, WrongKnowledge,
                   WrongQuestion, WrongNotePage, WrongCourse, WrongRank)
from up366 import (Up366Error, Up366AuthError, Up366APIError,
                   Up366RequestError, Up366HTTPError, Up366ResponseError)
import up366
assert up366.up366 is Up366          # up366 是 Up366 类的别名
```

> 上述模型已由集成提交从 `up366` 顶层重导出；也可按子模块导入（`from up366.book import Book, Chapter`、`from up366.job import Job, JobStatus`、`from up366.wrongnote import ...`）。详见第 5 节。

## 2. Quick start

```python
from up366 import login
session = login("your_account", "your_password")   # Up366 会话
for c in session.courses():                        # list[dict]
    print(c.get("courseId"), c.get("courseName"))
jobs = session.jobs(9222783)                       # list[Job]
for job in jobs:
    print(job.job_id, job.name, "单词PK" if job.is_word_pk else "普通")
info = session.job_info(9222783, jobs[0].job_id, jobs[0].content_id, jobs[0].book_id)
pk = next((j for j in jobs if j.is_word_pk), None)
if pk is not None:
    print(session.submit_word_pk(pk, score=8,
        correct_words=["apple", "banana"], wrong_words=["pear"]))
    # 也可先取词表：words = session.bucket_words(bucket_id)
```

`submit_word_pk` 词表可为字符串或 dict；默认构造**完整 `wordInfos`**（见 4.3）。

## 3. 认证、凭证、Cookie、TGT / ST

### 3.1 `login(username, password)`（`client.py`）

1. `password_hash = md5(md5(password) + "fa&s*l%$k!fq$k!ld@fjlk")`，salt 硬编码。
2. `ut = md5(ts)[:10] + md5(username + hash + "loginEncryptionParameter" + ts) + md5(ts)[10:]`，`ts = md5(毫秒时间戳)`。
3. `POST https://sso.up366.cn/v2/tickets-v3`，form `{username, password(=hash), ut}`，此时 `u3t` 为空串。
4. 成功码 **`result.code == -26`**，取 `data.tgt` 为 TGT、`data.uType` 存入 `user_type`（int）。
5. 登录响应 Cookie 全部存入 `session.cookies`。

`client_id` 取自环境变量 `UP366_CLIENT_ID`，未设置时由本机 MAC（`uuid.getnode()`）`sha1` 后转大写。本库**不校验** `uType`（官方客户端校验 `==102`），只原样存入。

### 3.2 会话字段与 7 个请求头

| 字段 | 类型 | 说明 |
|---|---|---|
| `username` / `password_hash` | `str` | 账号 / 密码哈希（**不保存明文 `password`**） |
| `tgt` / `client_id` | `str` | TGT（→ `u3t` 头）/ 设备 id（→ `clientId` 头） |
| `user_type` | `int \| None` | `data.uType`（学生通常 102） |
| `cookies` | `httpx.Cookies` | 会话 Cookie；**每个响应都 `update` 回来** |
| `headers` | `dict`（property） | 每次调用重新生成 7 个头 |

`headers`：`user-agent: PC-Up366-Student 6.11.0`、`x-requested-with: PC`、`X-App-Name: student-pc`、`clientId`、`u3r: uuid4().hex`（每次新生成）、`u3t: tgt`、`Authorization: ""`（恒空串）。

### 3.3 TGT / ST 自动换票

业务 host 的主要凭证是**登录 Cookie**：`UP366-C`、`CASTGC`（`Domain=.up366.cn`）。2026-11-05 实测仅 Cookie 即可访问 `courses` / `jobs` / `job_info` / `books` / `finished_tasks` / `wrongnote`，而**仅 `u3t` 返回 `-21`**；`SESSION` 只作用于 `sso.up366.cn`，业务 host 收不到也不需要。`u3t` 仍由 PC 客户端随请求发送，并在 Cookie 缺失/过期、业务返回 `result.code in (-21, -22)` 时用来换一次性 ST。换票流程：① `POST sso /v1/serviceTicket`，form `{service:<失败请求完整 URL>}`；② 期望 `code == -24`，取 `data.serverTicket`；③ 把 `?ticket=<ST>`（已有 query 用 `&ticket=`）拼回原 URL **重放一次**（`retry_auth=False`）。手工构造 `Up366(...)` 时必须带入登录 Cookie（至少 `UP366-C` / `CASTGC`）。请求层统一做 `form_encode`，非 200 抛 `Up366HTTPError`，`code != 0` 抛 `Up366APIError`。

### 3.4 安全警告

> - **绝不要把账号、密码、TGT、Cookie 写进仓库 / 日志 / 截图 / 版本控制**；示例账号密码换成你自己的。
> - **只用你本人有权使用的账号**，遵守平台条款；仅供学习 / 自用，不用于绕过限制或刷分。
> - `tgt` / `cookies` 等价于登录凭证，泄露即账号被他人登录；`password_hash` 也应按敏感信息处理。
> - 默认请求生产域名 `*.up366.cn`，会真实访问线上服务，勿测试他人账号或高频调用。
> - `password_hash` 仅为「未来重登」保留，**本库无重登 / 自动刷新 TGT 方法**（见第 9 节）。
> - 写接口默认 **dry-run**：`dry_run=True` 时零请求（即便 `confirm=True` 也不发）；须 `dry_run=False` **且** `confirm=True` 才真正发送（见第 8 节）。

## 4. Up366 API 参考

### 4.1 速查表（全部公开方法）

「组」：`现有` = 已稳定；`本批新增` = 本轮新加入，签名以本表为准。

| 组 | 方法 | 返回 |
|---|---|---|
| 现有 | `login(username, password)` | `Up366` |
| 现有 | `courses()` | `list[dict]` |
| 现有 | `jobs(course_id)` | `list[Job]` |
| 现有 | `job_info(course_id, job_id, content_id, book_id)` | `dict` |
| 现有 | `bucket_words(bucket_id)` | `list[dict]` |
| 现有 | `submit_word_pk(job, score, correct_words, wrong_words, *, role_name="Red", duration="", compact=False, allow_get_fallback=True)` | `dict` |
| 现有 | `headers`（property） | `dict[str,str]`（7 个头） |
| 本批新增·课本 | `books()` | `list[Book]` |
| 本批新增·课本 | `book_info(book_id, *, original_book_id="")` | `Book` |
| 本批新增·课本 | `book_tree(book_id, *, version="")` | `dict` |
| 本批新增·课本 | `custom_chapters(book_id, course_id)` | `dict` |
| 本批新增·课本 | `personalized_chapters(book_id)` | `dict` |
| 本批新增·课本 | `offline_books()` | `list[str]` |
| 本批新增·课本 | `book_plan(course_id, book_id, *, book_type=1)` | `list[dict]` |
| 本批新增·作业 | `finished_tasks(book_id)` | `list[dict]` |
| 本批新增·作业 | `answer_detail(params, *, use_v2=True)` | `dict` |
| 本批新增·作业 | `test_all_data(test_id, uid)` | `dict` |
| 本批新增·作业 | `paper_zip_info(paper_id)` | `dict` |
| 本批新增·作业 | `paper_detail(paper_id)` | `str`（XML 字符串） |
| 本批新增·作业 | `chapter_questions(book_id, chapter_id, file_id)` | `list[dict]` |
| 本批新增·作业 | `task_linked(book_id, task_id, *, type=1)` | `list[dict]` |
| 本批新增·作业 | `job_status(course_id, job_id, content_id, book_id)` | `JobStatus` |
| 本批新增·作业 | `submit_task_score(job, row, *, submit_type="0", dry_run=True, confirm=False, verify=True)` | `dict` |
| 本批新增·作业 | `submit_online_test(row, *, dry_run=True, confirm=False)` | `dict` |
| 本批新增·错题 | `wrong_subjects()` | `list[WrongSubject]` |
| 本批新增·错题 | `wrong_subject_stat(*, book_id="0", stage_id, subject_id, start_time=None, end_time=None)` | `WrongSubjectStat` |
| 本批新增·错题 | `wrong_books(*, stage_id, subject_id)` | `list[WrongBook]` |
| 本批新增·错题 | `wrong_knowledge(*, book_id="0", stage_id, subject_id, start_time=None, end_time=None, order_type="desc", order_field="wrongTimes", page=1, page_size=100)` | `WrongNotePage` |
| 本批新增·错题 | `wrong_courses(*, stage_id, subject_id)` | `list[WrongCourse]` |
| 本批新增·错题 | `wrong_rank(*, course_id, order_type="desc", order_field="wrongCount", page=1, page_size=100)` | `WrongNotePage` |
| 本批新增·错题 | `wrong_killed_questions(*, book_id, uid, stage_id, subject_id, kill_flag, start_time=None, end_time=None, page=1, page_size=10)` | `WrongNotePage` |
| 本批新增·错题 | `wrong_question_show(*, stage_id, subject_id, book_id, qtype_id, knowledge_id, start_time=None, end_time=None, page=1, page_size=10)` | `WrongNotePage` |
| 本批新增·错题 | `wrong_kill_paper(*, book_id, stage_id, subject_id, start_time=None, end_time=None)` | `dict`（读） |
| 本批新增·错题 | `submit_wrong_question(payload, *, dry_run=True, confirm=False)` | `dict` |

> `Up366(...)` 构造为全关键字：`username, password_hash, tgt, client_id, user_type=None, cookies=None`；普通使用请通过 `login()` 创建。

### 4.2 `login` / `courses` / `jobs` / `job_info` / `bucket_words`

| 方法 | 请求 | 返回 | 主要异常 |
|---|---|---|---|
| `login` | form `{username, password(hash), ut}`；`code=-26` 取 `data.tgt` | `Up366`（带 cookies） | 网络失败 → `Up366Error`；非 JSON / 非 200 / `code!=-26` / 缺 `data.tgt` → `Up366AuthError` |
| `courses()` | `{status:1, "pager.pageSize":999}` | `data.list` 原样 | 缺 `data` / 非 list → `Up366ResponseError`；业务码非 0 → `Up366APIError` |
| `jobs(course_id)` | `{courseId, pageIdFlag:1, version:""}` | `data.allTaskList` 每项经 `Job.from_dict` 包装 | 非 list → `Up366ResponseError` |
| `job_info(...)` | `{courseId, jobId, studyTaskId, contentId, bookId}` | `data` 对象（含 `taskStatus` / `paperId`），**不解析**直接返回 | 非 dict → `Up366ResponseError` |
| `bucket_words(bucket_id)` | `{source:1, bucketId}` | 展平 `data.contentList[].entryList[]` 词条 | 缺 `data` 对象 → `Up366ResponseError` |

`job_info` 参数可直接用 `Job` 属性；`bucket_words` 按服务端顺序展平，可喂给 `submit_word_pk`。

### 4.3 `submit_word_pk(...)`（POST 优先 / GET 兜底）

```python
submit_word_pk(job: Job, score: int,
               correct_words: Sequence[Mapping | str],
               wrong_words: Sequence[Mapping | str], *,
               role_name: str = "Red", duration: str = "",
               compact: bool = False, allow_get_fallback: bool = True) -> dict
```

前置校验（**发送前**）：`job` 非 `Job` → `TypeError("job must be a Job instance")`；`job.is_word_pk` 假 → `ValueError("job is not a word PK job")`；缺 `course_id`/`job_id` → `ValueError("job is missing courseId or jobId")`。非 PK 作业在**发出任何网络请求之前**被拒绝。

- **POST 为主**：端点 `word_pk_submit` 标记 `write=True, fallback="GET"`，默认 `POST https://wordsbtl-api.up366.cn/client/wordsbtl/student/submit`，参数经 `form_encode` 作表单提交。
- **GET 兜底**：仅当返回 **HTTP 404 / 405** 且 `allow_get_fallback=True` 时改用 GET 并把参数拼到 query；GET 分支**只携带紧凑词记录**。其它 `Up366HTTPError` 直接抛出。
- **参数**：`courseId`、`studyTaskId=job.job_id`、`roleName`、`roleType="0"`、`submitJson`（紧凑 JSON）、`wrongLength=len(wrong_words)`、`score`。
- `submitJson`：`{"totalNum": 词条总数, "rightNum": len(correct_words), "score": score, "wordInfos": [...], "duration": duration}`。
- **完整 `wordInfos`（默认 `compact=False`）**：`words.py` 的 `build_word_infos()` 构造，每项含 `entryId, en, cn, phonetic, audio, answer, answerStatus, answerTime, sourceType, localId`；正确 `answerStatus=1` / 错误 `=0`；字符串项 → `{"entry": s, "answer": s}`，`Mapping` 浅拷贝。
- **紧凑 `wordInfos`（`compact=True` 或 GET 兜底）**：每项只带 `entryId` 与 `answerStatus`，用于缩短 URL / 兼容旧 GET。
- 返回 `data` 为 dict/list 时原样返回，否则 `{}`。

> GET 兜底把 `submitJson` 拼进 query，词表大时 URL 很长，可能被网关截断（见第 11 节）；POST 通道不受此限。

### 4.4 本批新增 · 课本 API（`BookAPI`）

只读查询；参数 / 返回见第 6 节 `book-api` / `studytask-api`，字段级见 `recon/api.md §1.4 / §1.12`。

- `books()` 我的书架（PC 翻书 V11）→ `list[Book]`；`book_info(book_id, *, original_book_id="")` 单本详情 → `Book`（含 `packType`）。
- `book_tree(book_id, *, version="")` 整棵目录树（章 / 页 / 任务），`version=""` 取最新；`custom_chapters(book_id, course_id)` 自定义章节（`customFlag==1` 时才有意义）；`personalized_chapters(book_id)` 个性化章节。
- `offline_books()` 可离线下载教材 ID 白名单 → `list[str]`；`book_plan(course_id, book_id, *, book_type=1)` 课程学习计划（章节开关）→ `list[dict]`（`course_id` 为 `None` / `<=0` 时不发请求，直接返回 `[]`）。

### 4.5 本批新增 · 普通作业 API（`JobAPIMixin`）

只读查询 + 两类写接口（写接口默认 dry-run，见第 8 节）。

- 只读：`finished_tasks(book_id)` 已完成任务分数（只传 `bookId`；H5 用 POST、PC 用 GET，网关两条路由都接受，本库走 POST）；`answer_detail(params, *, use_v2=True)` 作答详情（两版参数相同：`{courseId,uid,bookId,taskId}`；`use_v2=True` → POST `/v2`，`use_v2=False` → GET 非 v2；响应均为 `data.paperInfo` + `data.userAnswer`）；`test_all_data(test_id, uid)` 测评全量数据（读 `data.testInfo.paperId`）；`paper_zip_info(paper_id)` 试卷包地址 / 版本；`paper_detail(paper_id)` 返回 **`data.paperData` 的 XML 字符串**（非 dict）；`chapter_questions(book_id, chapter_id, file_id)` 章节题目；`task_linked(book_id, task_id, *, type=1)` 任务关联目录；`job_status(course_id, job_id, content_id, book_id)` → `JobStatus`（复用 `job_info` 端点、**不新增端点行**，把 `data` 交给 `JobStatus.from_dict()`，返回模型而非原始 dict；请求体与 `job_info` 完全相同，`studyTaskId` 恒等于 `job_id`）。
- 写：`submit_task_score(job, row, *, submit_type="0", dry_run=True, confirm=False, verify=True)`（`verify=True` 提交成功后回读 `job_status()` 校验，不自动重提）；`submit_online_test(row, *, dry_run=True, confirm=False)`。

### 4.6 本批新增 · 错题本 API（`WrongNoteAPI`）

> 本组已按 H5 6.12.1 源码与真实账号只读 e2e 核实：**登录 Cookie 或一次性 ST `?ticket=` 任一即可鉴权，`u3t` 单独无效**；10 个端点的 body/query/响应见 `recon/wrongnote-h5.md`。分页类方法返回 `WrongNotePage`。

- `wrong_subjects()` → `list[WrongSubject]`；`wrong_subject_stat(*, book_id="0", stage_id, subject_id, start_time=None, end_time=None)` → `WrongSubjectStat`；`wrong_books(*, stage_id, subject_id)` → `list[WrongBook]`；`wrong_courses(*, stage_id, subject_id)` → `list[WrongCourse]`。
- 分页：`wrong_knowledge(...)`（`order_field` 默认 `wrongTimes`）、`wrong_rank(*, course_id, ...)`（`order_field` 默认 `wrongCount`）→ `WrongNotePage`；两者都把 pager 同时放进 form body 与 query（与 H5 一致）。
- 题包：`wrong_killed_questions(...)`（已消灭）、`wrong_question_show(...)`（剩余）→ `WrongNotePage`，`items` 为 `WrongQuestion`；`wrong_kill_paper(...)`（待消灭题包，**读**）→ `dict`。题包接口的 `WrongNotePage.raw` 保留完整 `data`（`pathPrefix` / `paperData` / `answerData` / `userAnswerData`），`items` 只是 `list` 的模型化视图。
- 写：`submit_wrong_question(payload, *, dry_run=True, confirm=False)`；payload 固定为 `uid/result/startTime/endTime/seconds/rightQstIds` 六项。

## 5. 模型参考

模型由对应方法返回；`Job` / `JobStatus` / `Book` / `Chapter` 字段据源码列出，错题本模型为薄封装（保留 `raw` / `to_dict()` / `from_dict()`，`WrongNotePage` 另有 `items` / `has_next`），均不臆造未验证字段。全部模型现已由集成提交从 `up366` 顶层重导出（见第 1 节），也可按子模块导入：

```python
from up366.book import Book, Chapter
from up366.job import Job, JobStatus
from up366.wrongnote import (WrongSubject, WrongSubjectStat, WrongBook, WrongKnowledge,
                            WrongQuestion, WrongNotePage, WrongCourse, WrongRank)
```

### 5.1 `Job` 与 `JobStatus`（`job.py`）

`Job` 由 `jobs()` 原始字典构造，字段均为只读 property：

| 属性 | 来源 / 规则 |
|---|---|
| `raw` | 输入字典的拷贝 |
| `ext_param` / `xot_param` | 解析 `raw.extParam` / `ext_param["xotParam"]`（dict 原样 / JSON 串反序列化 / 否则 `{}`） |
| `job_id` / `study_task_id` | `raw.jobId` 或 `raw.studyTaskId`；二者相等，提交时作 `studyTaskId` |
| `name` | `raw.jobName` → `raw.name` → `""` |
| `course_id` / `content_id` | `raw.courseId` 非 None 则用它，否则 `ext_param.courseId` / `raw.contentId` |
| `book_id` / `chapter_id` / `task_score` | `ext_param.<key>` 非 None 优先，否则回退 `raw.<key>` |
| `type` / `begin_time` / `end_time` | `raw.type` / `raw.beginTime` / `raw.endTime` |
| `is_word_pk` | `bool(xot_param.get("pkBucketId"))` 或 `xot_param` 含 `wordpkPlayType` 键 |

成员：`Job(raw)` / `Job.from_dict(raw)`、`to_dict()`（`raw` 拷贝）、`__repr__`。命中 `{"pkBucketId":...}` 或 `{"wordpkPlayType":...}` 判为 PK，不命中 `{"reportType":0}`。

**本批新增 · `Job` 扩展属性**：`paper_id` / `study_task_pid` / `task_status` / `stu_status` / `submit_time`（均 `ext_param.<key>` 优先、否则 `raw.<key>`）、`task_id`（`ext_param.taskId` 优先、否则 `raw.contentId`，作 `taskId`）、`is_finished`（`task_status==1` 或 `stu_status>1`）。

**本批新增 · `JobStatus`**（`job.py`）：`job_status()` 把 `job/info` 的 `data` 经 `from_dict()` 构造；只读属性 `task_status` / `submit_time` / `stu_status` / `is_finished`（规则同上），另有 `to_dict()`。

### 5.2 课本模型（`book.py`）· 本批新增

`Book`（`books()` / `book_info()` 返回）只读 property：`book_id` / `goods_id` / `book_name` / `cover` / `vertical_cover` / `pack_type` / `is_2025_reader` / `start_date` / `end_date` / `valid_days` / `stage_id` / `subject_id` / `category_type` / `file_id` / `pc_file_id` / `pc_version` / `resource_list`；`Chapter`（目录节点）：`id` / `parent_id` / `chapter_id` / `name` / `type` / `is_close`。二者均有 `from_dict()` / `to_dict()`；`Book.book_id` 与 `Book.goods_id` 相互独立、不互相回退。

### 5.3 错题本模型（`wrongnote.py`）· 本批新增（8 个）

| 模型 | 由谁返回 / 状态 | 含义 |
|---|---|---|
| `WrongSubject` / `WrongSubjectStat` | `wrong_subjects()` / `wrong_subject_stat()` | 学科条目 / 学科统计 |
| `WrongBook` / `WrongKnowledge` | `wrong_books()` / `wrong_knowledge()` 分页 | 教材条目 / 知识点条目 |
| `WrongQuestion` | `wrong_killed_questions()` / `wrong_question_show()` 的 `items` | 题目条目（薄封装，仅 `question_id`，其余字段在 `raw`） |
| `WrongNotePage` | `wrong_knowledge()` / `wrong_rank()` / 两个 show 方法 | 分页容器：`items`（模型实例）+ `pager` 字段 / `has_next` / `offset`；空 `data={}` 返回空页 |
| `WrongCourse` / `WrongRank` | `wrong_courses()` / `wrong_rank()` 分页 | 课程条目 / 排行条目（`WrongRank` 暴露 `realname` / `wrong_count` / `killed_count` / `recent_killed_count`） |

## 6. Endpoint 全表

所有业务请求带第 3.2 节 7 个头与 `cookies`；响应按 `result.code` 判断（0 成功、`-21/-22` 自动换 ST 重放一次）。默认超时 `DEFAULT_TIMEOUT = 20.0` 秒。目录见 `endpoints.py: ENDPOINTS`（32 条冻结 `Endpoint`：默认 `POST` / `body=form` / `write=False`，可带 `fallback` / `write`）。下表按 host 分组（顺序：sso、course-api、studytask-api、book-api、dsb-api、xot-api、qlib-api、study-api、wordsbtl-api、words-v2-api、wrongnote-api）；sso 两条为 `client.py` 常量，不在字典内。`params` 为请求参数语义，`returns` 为取用字段。

| Host | 方法 | Path | params | returns | 对应方法 / 备注 |
|---|---|---|---|---|---|
| sso | POST | `/v2/tickets-v3` | `{username, password(hash), ut}` | `data.tgt` / `data.uType` | 登录 `code=-26` |
| sso | POST | `/v1/serviceTicket` | `{service:<完整URL>}` | `data.serverTicket` | 换 ST `code=-24`，`-21/-22` 自动 |
| course-api | POST | `/client/course/list-of-student/v2` | `{status:1, pager.pageSize:999}` | `data.list` | `courses()` |
| studytask-api | POST | `/client/student/course/job` | `{courseId, pageIdFlag:1, version:""}` | `data.allTaskList` | `jobs()` |
| studytask-api | POST | `/client/student/job/info` | `{courseId, jobId, studyTaskId, contentId, bookId}` | `data` 对象 | `job_info()`；`job_status()` 复用本端点并解析为 `JobStatus` |
| studytask-api | POST | `/client/book/plan` | `{courseId, bookId, bookType:1}` | `data` 数组 | `book_plan()` |
| book-api | POST | `/client/books/buy-book` | 空 body（`body=none`） | `data` 数组 | `books()` |
| book-api | POST | `/client/v2/book/info` | `{bookId, bookType:1, originalBookId}` | `data` 对象 | `book_info()` |
| book-api | POST | `/client/books/tree` | `{bookId, version, bookType:1}` | `data` 目录树（`version`/`chapters`/`pages`/`tasks`） | `book_tree()` |
| book-api | POST | `/client/custom/book/chapters` | `{bookId, courseId}` | `data` | `custom_chapters()` |
| book-api | POST | `/client/chapters/personalized/list` | `{bookId}` | `data` | `personalized_chapters()` |
| book-api | POST | `/front/offline/download/books` | 空 body（`body=none`） | `data.bookIds` | `offline_books()` |
| book-api | POST | `/client/chapters/list-question-file` | `{bookId, chapterId, fileId}` | `data.dirQuestions` | `chapter_questions()` |
| book-api | POST | `/client/task/linked` | `{bookId, taskId, type:1}` | `data.chapters` | `task_linked()` |
| dsb-api | POST（H5）/ GET（PC） | `/client/book/finished-task/list` | `{bookId}` | `data` 数组（`[].taskId` 为键） | `finished_tasks()`；两端都只传 `bookId`，本库走 POST |
| dsb-api | POST | `/client/student/task/answer/detail/v2` | `{courseId,uid,bookId,taskId}` | `data.paperInfo` + `data.userAnswer` | `answer_detail(use_v2=True)`；H5 变体（`courseId` 来自父路由，缺省报 `param_missing`） |
| dsb-api | GET | `/client/student/task/answer/detail` | `{courseId,bookId,taskId,uid}` | 同上 | `answer_detail(use_v2=False)`；PC 变体 |
| xot-api | POST | `/client/onlinetesting/alldata` | `{testId, uid}` | `data.testInfo.paperId` | `test_all_data()` |
| xot-api | POST | `/client/paper/zipinfo` | `{paperId}` | `data.downloadurl` / `data.fileId` | `paper_zip_info()` |
| xot-api | POST | `/client/onlinetesting/submit` | 整行成绩 + `testId` | `result.code` | `submit_online_test()`（write） |
| qlib-api | GET | `/client/paper/detail` | query `{paperId}` | `data.paperData`（XML 字符串） | `paper_detail()` |
| study-api | POST | `/client/task/score/submit/v2` | `{tasksJson, submitType, ut}` | `result.code` | `submit_task_score()`（write） |
| wordsbtl-api | POST→GET | `/client/wordsbtl/student/submit` | `{courseId, studyTaskId, roleName, roleType, submitJson, wrongLength, score}` | `data` | `submit_word_pk()`；仅 404/405→GET 且只带紧凑词记录 |
| words-v2-api | POST | `/client/sync/teaching/bucket/detail-info` | `{source:1, bucketId}` | `data.contentList[].entryList[]` | `bucket_words()` |
| wrongnote-api | POST | `/client/wrongnote/student/subject/list` | — | 学科列表 | `wrong_subjects()` |
| wrongnote-api | POST | `/client/wrongnote/student/subject/stat` | `bookId,stageId,subjectId,startTime,endTime` | 统计 | `wrong_subject_stat()` |
| wrongnote-api | POST | `/client/wrongnote/student/book/list` | `stageId,subjectId` | 教材列表（库为对齐 UI 另注入「全部」/「练习」哨兵项） | `wrong_books()` |
| wrongnote-api | POST | `/client/wrongnote/student/knowledge/list/v3` | body: `bookId,stageId,subjectId,startTime,endTime,orderType,orderField,pager.pageSize,pager.currentPage`；query: `pager.currentPage,pager.pageSize` | `data.list`/`pager` | `wrong_knowledge()` |
| wrongnote-api | POST | `/client/wrongnote/student/course/list` | `stageId,subjectId` | 课程列表 | `wrong_courses()` |
| wrongnote-api | POST | `/client/wrongnote/v2/rank/list` | body: `courseId,orderType,orderField,pager.pageSize,pager.currentPage`；query: `pager.currentPage,pager.pageSize` | `data.list`/`pager`（行含 `realname,wrongCount,killedCount,recentKilledCount`） | `wrong_rank()` |
| wrongnote-api | POST | `/client/wrongnote/kill/question/show` | `{bookId,uid,stageId,subjectId,killFlag,pager.pageSize,pager.currentPage[,startTime,endTime]}` + query pager | 题包 `data.{pathPrefix,paperData,answerData,userAnswerData,list,pager}` | `wrong_killed_questions()`（读） |
| wrongnote-api | POST | `/client/wrongnote/student/question/show` | `{stageId,subjectId,bookId,qtypeId,knowledgeId,startTime,endTime,pager.pageSize,pager.currentPage}` + query pager | 同上 | `wrong_question_show()`（读） |
| wrongnote-api | POST | `/client/wrongnote/student/questions/kill` | `{bookId,stageId,subjectId,startTime,endTime}` | 题包 `data.{pathPrefix,paperData,answerData}` | `wrong_kill_paper()`（**读**：取待消灭题包） |
| wrongnote-api | POST | `/client/wrongnote/student/question/submit` | `{uid,result,startTime,endTime,seconds,rightQstIds}` | `result.code` | `submit_wrong_question()`（write） |

> **wrongnote-api 整组**：路径与字段已由 H5 6.12.1 源码核实（`recon/wrongnote-h5.md`）；请求体里的 `pager###pageSize` 经 `form_encode` 展平为 `pager.pageSize`，H5 另把同样的 pager 放进 query。鉴权用登录 Cookie 或 ST `?ticket=` 任一即可（`u3t` 单独无效）：PC 客户端的 H5 webview 由 Electron 拦截 sso 换票（`preload/index.js@3640984` → `redirect://getServiceTicket`）。写接口仍会真实改动账号数据，务必先 dry-run。

## 7. 错误处理

全部继承 `Up366Error`（`errors.py`）；`Up366AuthError` / `Up366APIError` 附 `code` / `message`，`Up366HTTPError` 附 `status_code` / `url`。

| 异常 | 触发时机 | 典型信息 |
|---|---|---|
| `Up366Error` | 基类；`login()` 网络层失败 | `login request failed: ...` |
| `Up366AuthError` | 登录失败：非 JSON / 非 200 / `code!=-26` / 缺 `data.tgt` | `up366 login failed (code=-27): 用户名或密码错误` |
| `Up366APIError` | 业务码非 0（已处理 `-21/-22` 除外），或换 ST 未拿到 `-24` | `up366 api error (code=-4): 无权限` |
| `Up366RequestError` | HTTP 请求在拿到可用响应前失败 | `POST <url> failed: ...` |
| `Up366HTTPError` | HTTP 状态非 200 | `HTTP 502 <url>: ...` |
| `Up366ResponseError` | 响应结构不符：非法 JSON、非对象、缺 `data`、`data.<key>` 非 list、缺 `serverTicket` | `response did not contain a data object for list` |

关键业务码：`-21/-22` 没登录 / ST 校验失败（自动换 ST 重放一次）、`-24` 生成 ST 成功、`-26` 生成 TGT 成功（登录）、`-23` 没有 TGT、`-13` 重复作答 / 次数用尽（抛 `Up366APIError`、不自动重试）、`-27` 用户名或密码错误。

```python
from up366 import login, Up366Error, Up366AuthError, Up366APIError
try:
    jobs = login(account, pwd).jobs(course_id)
except Up366AuthError as e:
    print("登录失败", e.code, e.message)
except Up366APIError as e:
    print("业务错误", e.code, e.message)   # 含 -13 重复提交
except Up366Error as e:
    print("其它错误", e)
```

## 8. 示例

> 沿用第 2 节已登录的 `session`。

**8.1 只挑单词 PK 作业提交（`job.is_word_pk` 过滤）**

```python
for job in session.jobs(9222783):
    if not job.is_word_pk:
        continue
    print("提交", job.name, job.job_id)
    print(session.submit_word_pk(job, score=8,
        correct_words=[{"entryId": "E1", "entry": "apple"},
                       {"entryId": "E2", "entry": "banana"}],
        wrong_words=[{"entryId": "E3", "entry": "pear"}], role_name="Red"))
```

**8.2 非 PK 作业被本地拒绝（不发请求）**

```python
ordinary = next(j for j in session.jobs(9222783) if not j.is_word_pk)
try:
    session.submit_word_pk(ordinary, 1, ["apple"], [])
except ValueError as e:
    print(e)   # job is not a word PK job
```

该 `is_word_pk` 校验在任何网络调用之前完成。

**8.3 写接口 dry-run（默认零请求）**

写方法 `submit_task_score` / `submit_online_test` / `submit_wrong_question` 默认 `dry_run=True, confirm=False`：只构造并返回预览 payload（`endpoint` / `url` / `data`，`submit_task_score` 另有 `tasks_json`），**零网络请求**；须同时 `dry_run=False` **且** `confirm=True` 才真正发送，二者缺一不发。

```python
preview = session.submit_task_score(job, row, dry_run=True)   # 零请求，返回 payload
result = session.submit_task_score(job, row,
        dry_run=False, confirm=True, verify=True)             # 真正发送（verify 回读校验）
# 错题写同理（payload 字段见 4.6，务必先 dry-run 复核）：
preview = session.submit_wrong_question(payload, dry_run=True)
# result = session.submit_wrong_question(payload, dry_run=False, confirm=True)
```

## 9. 已知限制

> 2026-11-05 已用测试学生账号完成**只读** e2e；下面标「已确认」的条目均已线上验证，未标注的仍是逆向推断。

- **`wrongnote-api` 鉴权已确认**：H5 6.12.1 的 axios 封装在 `-21/-22` 时向响应里的 `data.serverTicketUrl` POST `{service:<原 URL>}`，再把 `?ticket=` 拼回原 URL 重放；PC 客户端的 webview 由 Electron 拦截该 sso 请求并用 PC TGT 换票。2026-11-05 实测矩阵：仅 Cookie=`0`、仅 `?ticket=`=`0`、仅 `u3t`=`-21`、无凭证=`-21` → **Cookie 或 ST 任一即可，`u3t` 单独无效**。本库同时带 Cookie/u3t 并保留 ST 重试，可用。
- **错题写接口 payload 已确认**：`submit_wrong_question` 的 `uid/result/startTime/endTime/seconds/rightQstIds` 六个字段来自 H5 `question/submit`（`recon/wrongnote-h5.md` §3.3）；`questions/kill` 是**读**接口，已从写接口改为 `wrong_kill_paper()`。写接口仍会真实改动账号数据，默认 dry-run。
- **`answer_detail` 参数已确认**：两版都是 `{courseId,uid,bookId,taskId}`；`use_v2=True` → POST `/v2`（H5 的 `courseId` 来自父路由，缺省报 `param_missing`），`use_v2=False` → GET 非 v2（PC 调用点 `main/index.js@3768493`）。`taskId` 取 `contentId`（不是 `jobId`）；响应均为 `data.paperInfo` + `data.userAnswer`。2026-11-05 用真实账号只读实测通过。
- **`finished_tasks` 方法与参数已确认**：PC 用 GET、H5 用 POST，两端都只传 `bookId`；2026-11-05 真实账号实测两种方法返回完全相同的行，额外 `courseId` 被忽略。本库走 POST，删除了无依据的 `courseId` 参数。
- **无隐式缓存**：官方客户端对 `book/info` / `task/linked` / 离线书籍做 2h 或 SQLite 缓存，本库每次发真实请求，不缓存。
- **无自动重登 / TGT 刷新**：登录态过期需重新 `login()`；`password_hash` 已存但无重登方法。
- **写接口默认不真实写入**：默认 `dry_run=True` 且零请求，须 `dry_run=False` + `confirm=True`（第 8 节）；单词 PK 提交是唯一默认即发送的写入口。
- **教材阅读器版本**：`packType` `9` = 2025 版、`7` = 2019 版、`<7` = 2016 版；本库只取数据，不实现阅读器渲染。
- **未实现**：短信 / 扫码 / 微信 / QQ / token 自动登录、词王争霸 / 活动模式提交、异步 / 连接池 / 会话持久化。

## 10. 测试

```bash
python -m pytest -q     # 仓库根目录
```

`pyproject.toml` 已配 `pythonpath=["src"]`、`testpaths=["tests"]`，无需额外安装。测试用 `monkeypatch` / `httpx.MockTransport` 打桩，**不发真实网络请求**，覆盖核心语义：登录与凭证存储、7 个请求头、Cookie 捕获、`-21/-22` 换 ST 重放一次、非 200 / 业务错误码、响应结构校验、`Endpoint` 冻结与目录键、`form_encode`、`build_word_infos`、`Job` / `JobStatus` / `Book` 模型、单词 PK POST 提交 / GET 兜底 / 非 PK 拒绝、词桶展平，以及课本 / 普通作业 / 错题本各组方法。各组新增方法由对应 `tests/test_*.py` 覆盖，全部本地打桩、不触网。每次功能修改后须检查测试与当前版本是否相符（见 `AGENTS.md`）。

## 11. 故障排查

| 现象 | 原因 | 处理 |
|---|---|---|
| `-21` / `-22`（没有登录 / ST 校验失败） | 登录 Cookie 缺失/过期，或该接口要求一次性 ST | 已自动 `POST /v1/serviceTicket` 并 `?ticket=` 重放**一次**；仍失败抛 `Up366APIError`。先确认 `cookies`（`UP366-C`/`CASTGC`）完整，再确认 `tgt` 有效 |
| 换 ST 失败 | 未返回 `code == -24` 或缺 `data.serverTicket` | 抛 `Up366APIError` / `Up366ResponseError`；多为 TGT 失效，需重新 `login()`（无自动重登） |
| `-23`（没有 TGT） | `serviceTicket` 需要有效 `u3t` | 手工构造 `Up366` 时 `tgt` 为空触发；请通过 `login()` 创建或填入有效 TGT |
| 一直被判未登录 | 丢了会话 Cookie | `UP366-C / CASTGC / SESSION` 须随每次请求携带；本库自动 `cookies.update`，手工构造会话要自己传入 |
| `-13`（重复作答 / 次数用尽） | 服务端按 `(uid, studyTaskId)` 判重，同一作业只有一次机会 | 作为 `Up366APIError(code=-13)` 抛出，**不自动重试**；勿重复提交同一 job |
| `Up366HTTPError` 非 200 | 服务端返回 HTTP 错误状态 | 直接抛出；仅 `submit_word_pk` 在 404/405 转 GET 兜底 |
| POST→GET 兜底 / URL 过长 | `submit_word_pk` POST 被拒（404/405）或 GET 把 `submitJson` 拼进 query | 默认走 POST 无此问题；GET 兜底用紧凑词记录，控制词表规模 / 更少字段 |
| `ValueError: job is not a word PK job` | 传了普通作业给 `submit_word_pk` | 先用 `job.is_word_pk` 过滤；发送前本地校验 |
| `TypeError: job must be a Job instance` | 传了 dict 而非 `Job` | `jobs()` 返回 `Job`；`courses()` 返回 dict |
| `Up366ResponseError: ... is not a list` | 服务端返回结构变化 | 本库按 `data.list` / `data.allTaskList` / `data.list+pager` 严格解析 |
| 错题本接口返回异常 / 未登录 | TGT 过期、ST 换票失败或 payload 不符 | 见第 9 节；读接口会自动换 ST，写接口先 `dry_run=True` 复核 |

> 更多协议细节见 `recon/api.md`、`recon/feature-word-pk.md`、`recon/frontend.md`、`recon/readaloud-runbook.md`。
