# up366 库 POST 方法重写 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `up366` 库中适合 POST 的请求改成 POST，优先解决单词 PK 提交因 GET URL 过长而无法携带完整 `wordInfos` 的问题，同时保留对仅支持 GET 接口的兼容回退。

**Architecture:** 增加一个轻量 endpoint catalog，集中声明每个接口的首选方法、回退方法和写入属性；`Up366._request()` 统一处理 form 编码、GET query、-21/-22 ST 重放和只读接口的方法回退。单词 PK 提交改为 POST + 完整展示字段；仅当服务端明确不支持 POST（405/404）时才回退 GET，且 GET 只能使用 compact `wordInfos`。

**Tech Stack:** Python 3.13、httpx、pytest、现有 `src/up366` 包。

---

## 背景与已验证事实

### 1. 当前问题

本次 `11.4词汇PK/单词PK` 的提交用了 compact `wordInfos`：

```json
{"answerStatus": 1, "entryId": 7589}
```

服务端原样存储，结果详情接口 `GET /client/wordsbtl/student/detail` 返回 89 条同样的对象，因此详情页没有 `en/cn/phonetic/audio/answer` 可渲染。

正常 PK 的 detail 返回：

```json
{
  "answerStatus": 1,
  "phonetic": "",
  "answer": "regardless of",
  "answerTime": 1792141833479,
  "sourceType": 1,
  "en": "regardless of",
  "cn": "Phrase不管；不顾",
  "audio": "https://word-cdn-alyun.up366.cn/entryfilesv2/7465/....mp3",
  "localId": 141,
  "entryId": 7465
}
```

### 2. 原版客户端如何构造 wordInfo

证据：`work/flipbook-pk/js/6c688bf3-829dffba.js` 的 `E(t)` / `x(t)`：

```js
function E(t, audioBase = "") {
  var n = {};
  n.phonetic = t.usPhonetic;
  n.sourceType = t.sourceType;
  n.entryId = t.entryId;
  n.en = t.entry;
  if (window.location.hash != "#/CswpPkIndex") n.cn = S(t);
  if (audioBase) n.audio = T(t, audioBase);
  return n;
}

function x(t, audioBase = "") {
  var out = [];
  t.forEach(function (word, index) {
    var info = E(word, audioBase);
    info.localId = index;
    out.push(info);
  });
  return out;
}
```

其中：

- `S(t)` 优先使用 `newPropList[].prop + senseType==0 的 paraphrase`，否则用 `paraphrase`。
- `T(t, base)` 优先 `usVoiceFile`，其次 `enVoiceFile`；不是 http 开头时拼 `base + "/" + file`。
- `localId` 是词在本次列表中的下标。
- 答对/答错时再补 `answer`、`answerStatus`、`answerTime`。

### 3. 方法矩阵（原版）

| 接口 | 原版方法 | 结论 |
|---|---|---|
| `/v2/tickets-v3` | POST | 保持 |
| `/v1/serviceTicket` | POST | 保持 |
| `/client/course/list-of-student/v2` | POST | 保持 |
| `/client/student/course/job` | POST | 保持 |
| `/client/student/job/info` | POST（直接调用） | 保持，补 `studyTaskId` |
| `/client/sync/teaching/bucket/detail-info` | POST | 保持 |
| `/client/wordsbtl/student/submit` | **GET** | 改为 POST 优先，405/404 才回退 GET |
| `/client/book/finished-task/list` | GET（`getByName`） | 可 POST 优先 + GET 回退（待实测） |
| `/client/student/task/answer/detail` | GET | 可 POST 优先 + GET 回退（待实测） |
| `/client/paper/detail` | GET | 可 POST 优先 + GET 回退（待实测） |
| `/client/wordsbtl/student/detail` | GET | 只读；可 POST 优先 + GET 回退 |
| `/client/wordsbtl/student/complete/info` | GET | 只读；可 POST 优先 + GET 回退 |
| `/client/wordsbtl/student/start` | GET | 只读；可 POST 优先 + GET 回退 |
| `/client/msg/list-of-pc` | POST | 保持 |

> 原则：**写接口不做自动 GET 回退**。只有 405/404 这种“请求根本没被处理”的情况才允许回退；业务错误（如 `-13`）必须原样抛出，避免重复提交。

### 4. 大小约束

89 个完整词条的 GET URL：

```text
完整详情 URL ≈ 40,714 字节
compact URL  ≈ 5,231 字节
```

GET 提交不能承载完整详情；POST body 没有这个限制。因此单词 PK 必须优先 POST。

---

## Task 1: 建立 endpoint catalog 与 form 编码

**Files:**
- Create: `src/up366/endpoints.py`
- Modify: `src/up366/client.py`（`_request`）
- Test: `tests/test_transport.py`

**Step 1: 写失败测试**

```python
def test_form_encode_serializes_objects_and_replaces_dots():
    encoded = client_module._form_encode(
        {"a###b": {"x": 1}, "plain": "v"}
    )
    assert encoded["a.b"] == '{"x": 1}'
    assert encoded["plain"] == "v"
```

**Step 2: 运行测试确认失败**

Run: `python -m pytest tests/test_transport.py -q`
Expected: FAIL，`_form_encode` 不存在。

**Step 3: 最小实现**

`src/up366/endpoints.py`：

```python
from dataclasses import dataclass
from typing import Literal

BodyKind = Literal["form", "query", "json", "none"]

@dataclass(frozen=True)
class Endpoint:
    name: str
    base_url: str
    path: str
    method: str = "POST"
    body: BodyKind = "form"
    fallback: str | None = None
    write: bool = False

ENDPOINTS = {
    "courses": Endpoint("courses", "https://course-api.up366.cn", "/client/course/list-of-student/v2"),
    "jobs": Endpoint("jobs", "https://studytask-api.up366.cn", "/client/student/course/job"),
    "job_info": Endpoint("job_info", "https://studytask-api.up366.cn", "/client/student/job/info"),
    "bucket_words": Endpoint("bucket_words", "https://words-v2-api.up366.cn", "/client/sync/teaching/bucket/detail-info"),
    "word_pk_submit": Endpoint(
        "word_pk_submit",
        "https://wordsbtl-api.up366.cn",
        "/client/wordsbtl/student/submit",
        method="POST",
        fallback="GET",
        write=True,
    ),
}
```

`client.py` 增加：

```python
def _form_encode(data: Mapping[str, Any]) -> dict[str, str]:
    out: dict[str, str] = {}
    for key, value in data.items():
        key = key.replace("###", ".")
        out[key] = (
            json.dumps(value, ensure_ascii=False)
            if isinstance(value, (Mapping, list))
            else str(value)
        )
    return out
```

`_request` 增加 `endpoint: Endpoint | None` 或 `body_kind` 参数；POST 时用 `_form_encode(data)`。

同时把 HTTP 层错误显式化：

```python
class Up366HTTPError(Up366Error):
    def __init__(self, status_code: int, url: str, message: str):
        self.status_code = status_code
        self.url = url
        super().__init__(f"HTTP {status_code} {url}: {message}")
```

`_request` 在解析 JSON 前先检查 `response.status_code`；非 200 直接抛 `Up366HTTPError`，这样 405/404 才能被 method fallback 精确识别，不会被误当成 JSON 结构错误。


**Step 4: 运行测试**

Run: `python -m pytest tests/test_transport.py -q`
Expected: PASS。

**Step 5: 提交**

```bash
git add src/up366/endpoints.py src/up366/client.py tests/test_transport.py
git commit -m "refactor(up366): add endpoint catalog and form encoding"
```

---

## Task 2: 实现完整 wordInfo 映射

**Files:**
- Create: `src/up366/words.py`
- Test: `tests/test_words.py`

**Step 1: 写失败测试**

```python
def test_build_word_info_matches_client_display_shape():
    word = {
        "entryId": 7465,
        "entry": "trial",
        "paraphrase": "n. 试验；试用",
        "usPhonetic": "[ˈtraɪəl]",
        "usVoiceFile": "entryfilesv2/7465/audio.mp3",
        "sourceType": 1,
        "newPropList": [],
    }
    info = build_word_info(word, answer_status=1, index=3, answer="trial")
    assert info["entryId"] == 7465
    assert info["en"] == "trial"
    assert info["cn"] == "n. 试验；试用"
    assert info["phonetic"] == "[ˈtraɪəl]"
    assert info["audio"].endswith("/entryfilesv2/7465/audio.mp3")
    assert info["answer"] == "trial"
    assert info["answerStatus"] == 1
    assert info["localId"] == 3
```

**Step 2: 运行测试确认失败**

Run: `python -m pytest tests/test_words.py -q`
Expected: FAIL，`build_word_info` 不存在。

**Step 3: 实现 `words.py`**

```python
AUDIO_BASE = "https://word-cdn-alyun.up366.cn"

def word_cn(word: Mapping[str, Any]) -> str:
    parts: list[str] = []
    for prop in word.get("newPropList") or []:
        text = str(prop.get("prop") or "")
        for sense in prop.get("senseList") or []:
            if sense.get("senseType") == 0:
                text += str(sense.get("paraphrase") or "")
        if text:
            parts.append(text)
    return "；".join(parts) if parts else str(word.get("paraphrase") or "")

def word_audio(word: Mapping[str, Any], base: str = AUDIO_BASE) -> str:
    name = word.get("usVoiceFile") or word.get("enVoiceFile") or ""
    if not name:
        return ""
    if name.startswith("http"):
        return name
    return f"{base.rstrip('/')}/{name.lstrip('/')}"

def build_word_info(
    word: Mapping[str, Any],
    *,
    answer_status: int,
    index: int,
    answer: str | None = None,
    answer_time: int | None = None,
) -> dict[str, Any]:
    info = {
        "entryId": word.get("entryId"),
        "en": word.get("entry") or "",
        "cn": word_cn(word),
        "phonetic": word.get("usPhonetic") or word.get("enPhonetic") or "",
        "audio": word_audio(word),
        "sourceType": word.get("sourceType", 1),
        "localId": index,
        "answer": answer if answer is not None else (word.get("entry") if answer_status == 1 else ""),
        "answerStatus": answer_status,
        "answerTime": answer_time or int(time.time() * 1000) + index,
    }
    return info
```

**Step 4: 运行测试**

Run: `python -m pytest tests/test_words.py -q`
Expected: PASS。

**Step 5: 提交**

```bash
git add src/up366/words.py tests/test_words.py
git commit -m "feat(up366): build full word PK display records"
```

---

## Task 3: 补回 `bucket_words` 与 `job_info.studyTaskId`

**Files:**
- Modify: `src/up366/client.py`
- Test: `tests/test_words.py`、`tests/test_courses.py`

**Step 1: 写失败测试**

```python
def test_bucket_words_posts_bucket_id(...):
    session.bucket_words("BUCKET-1")
    assert captured["method"] == "POST"
    assert captured["data"] == {"source": 1, "bucketId": "BUCKET-1"}

def test_job_info_sends_study_task_id(...):
    session.job_info(1, "JOB", "CONTENT", "BOOK")
    assert captured["data"]["studyTaskId"] == "JOB"
```

**Step 2: 运行确认失败**

Run: `python -m pytest tests/test_words.py tests/test_courses.py -q`
Expected: FAIL。

**Step 3: 实现**

- `bucket_words(bucket_id)`：POST `bucket/detail-info`，`data={"source":1,"bucketId":bucket_id}`，展平 `contentList[].entryList[]`。
- `job_info` 请求体补 `"studyTaskId": job_id`。

**Step 4: 运行测试**

Run: `python -m pytest tests/test_words.py tests/test_courses.py -q`
Expected: PASS。

**Step 5: 提交**

```bash
git add src/up366/client.py tests/test_words.py tests/test_courses.py
git commit -m "fix(up366): restore bucket_words and job_info studyTaskId"
```

---

## Task 4: 单词 PK 改为 POST + 完整详情

**Files:**
- Modify: `src/up366/client.py`
- Modify: `src/up366/endpoints.py`
- Test: `tests/test_word_pk.py`

**Step 1: 写失败测试**

```python
def test_submit_word_pk_uses_post_and_full_details(...):
    ...
    assert captured["method"] == "POST"
    assert captured["headers"]["content-type"].startswith(
        "application/x-www-form-urlencoded"
    )
    submit = json.loads(captured["data"]["submitJson"])
    assert submit["wordInfos"][0]["en"] == "trial"
    assert submit["wordInfos"][0]["cn"]
    assert submit["wordInfos"][0]["audio"]
```

**Step 2: 运行确认失败**

Run: `python -m pytest tests/test_word_pk.py -q`
Expected: FAIL，当前仍是 GET。

**Step 3: 实现**

- `_build_word_infos` 改为调用 `words.build_word_infos`，默认完整字段。
- `submit_word_pk` 默认 `method="POST"`，`data` 使用 `_form_encode(params)`。
- 新增 `allow_get_fallback: bool = True`：
  - 仅当 POST 返回 HTTP 405/404 时，回退 GET；
  - GET 回退时强制 `compact=True`（`entryId + answerStatus`），并记录 warning；
  - 业务错误 `-13`/`-1`/`-99` 不自动回退，直接抛 `Up366APIError`。

**Step 4: 运行测试**

Run: `python -m pytest tests/test_word_pk.py -q`
Expected: PASS，覆盖 POST、GET fallback、业务错误不回退。

**Step 5: 提交**

```bash
git add src/up366/client.py src/up366/endpoints.py tests/test_word_pk.py
git commit -m "feat(up366): submit word PK via POST with full word details"
```

---

## Task 5: 只读接口 POST 优先 + GET 回退（可选 P2）

**Files:**
- Modify: `src/up366/client.py`
- Test: `tests/test_method_fallback.py`

**Step 1: 写失败测试**

```python
def test_readonly_post_405_falls_back_to_get(...):
    ...
    assert calls == [("POST", URL), ("GET", URL)]
```

**Step 2: 实现**

新增方法（按需）：

- `finished_tasks(book_id, course_id)`
- `task_answer_detail(**params)`
- `paper_detail(paper_id)`
- `word_pk_detail(course_id, study_task_id)`
- `word_pk_complete(course_id, study_task_id)`

统一调用 `_call_endpoint`：

```python
def _call_endpoint(self, endpoint: Endpoint, *, data=None, params=None):
    method = endpoint.method
    try:
        return self._request(method, endpoint.base_url + endpoint.path, data=data, params=params)
    except Up366HTTPError as exc:
        if endpoint.fallback and not endpoint.write and exc.status_code in (404, 405):
            return self._request(endpoint.fallback, ..., ...)
        raise
```

只有只读 endpoint 允许 `fallback`。写 endpoint（`write=True`）不自动回退。

**Step 3: 运行测试**

Run: `python -m pytest tests/test_method_fallback.py -q`
Expected: PASS。

**Step 4: 提交**

```bash
git add src/up366/client.py tests/test_method_fallback.py
git commit -m "feat(up366): prefer POST for read-only APIs with GET fallback"
```

---

## Task 6: 重写 `a.py`

**Files:**
- Modify: `src/a.py`
- Test: 只做 `--dry-run` 手动验证

**Step 1: 更新脚本**

- 使用 `bucket_words` 获取词库。
- 使用 `build_word_info` 生成完整 89 个词条。
- 调用 `submit_word_pk`（默认 POST，不再 `compact=True`）。
- 保留 `--dry-run`。
- 保留精确 jobId/时间窗匹配，避免提交到同名旧作业。
- 提交前打印 `total/score` 和首/末词条；提交后调用 `word_pk_detail` 检查 `en/cn/audio` 是否存在。

**Step 2: dry-run**

```bash
python -u src/a.py --dry-run
```

Expected：打印目标 job、89 个词、POST 计划，不发请求。

**Step 3: 真实提交（仅在新任务/测试任务上）**

不要在已提交的 11.4 任务上重复提交；重复会返回 `-13`。

**Step 4: 提交**

```bash
git add src/a.py
git commit -m "feat(up366): submit word PK with full display details"
```

---

## Task 7: 方法探测与文档

**Files:**
- Create: `tools/probe_methods.py`
- Create: `recon/methods.md`

**Step 1: 写探测脚本**

对每个 endpoint 发送：

- `OPTIONS` / `HEAD` 看允许方法；
- 只读接口用无效参数发 POST，观察是否 405/404/业务错误；
- **不要**用有效写 payload 探测写接口。

**Step 2: 运行并记录**

```bash
python tools/probe_methods.py --env up366test --readonly
```

Expected：输出 method matrix，写入 `recon/methods.md`。

**Step 3: 提交**

```bash
git add tools/probe_methods.py recon/methods.md
git commit -m "docs: record up366 API method compatibility"
```

---

## 验收标准

1. `submit_word_pk` 默认 POST，发送完整 `wordInfos`；在**未提交的测试任务**上验证 `detail` 返回包含 `en/cn/phonetic/audio/answer/answerTime/localId`。如果服务端拒绝 POST，记录 405/404 或业务错误，并保留 GET compact 回退；不得把“POST 成功”写成未经验证的事实。
2. POST 返回 405/404 时才回退 GET compact；业务错误不回退。
3. `bucket_words`、`job_info`、`courses`、`jobs` 正常。
4. 全量测试通过：`python -m pytest -q`。
5. `a.py --dry-run` 可打印 88 正 + 1 错、total 89 的计划。
6. 已提交的 11.4 任务不做自动重提；如需修复详情，单独用 POST 试验并取得用户确认。

## 风险

- 服务端可能只接受 GET：POST 会返回 405/404，必须靠 fallback。
- POST 写接口不做盲目重试，避免重复成绩。
- 完整 89 词 POST body 约 40KB；确认服务端/网关允许该 body 大小。
- 方法切换后 ST 的 `service` URL 必须使用最终 URL（GET 含 query）。
- 不要在当前已提交任务上反复试验；优先用测试账号/未提交任务。
