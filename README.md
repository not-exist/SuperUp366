# up366 — Python API client

天学网（up366）PC 学生端协议的最小 Python 实现：登录、课程 / 作业查询、教材、普通作业、错题本查询，以及单词 PK 成绩提交。

完整协议依据、加密分析与安全评估在 `main` 分支（研究档案）；本分支只放可安装的库与测试。

## 安装

```bash
pip install -e .              # 可编辑安装
```

Python >= 3.10，唯一运行时依赖 `httpx>=0.27`。

## 测试

```bash
python -m pytest -q           # 114 passed
```

## 快速开始

```python
from up366 import login

user = login("账号", "口令")
for job in user.jobs(9224394):
    print(job.job_id, job.name, job.status)
```

## 模块

| 文件 | 内容 |
|---|---|
| `client.py` | `login` / `Up366` / 会话 |
| `endpoints.py` | 端点目录（32 条） |
| `book.py` / `job_api.py` / `wrongnote.py` | 教材 / 普通作业 / 错题本 mixin |
| `job.py` | `Job` / `JobStatus` 模型 |
| `form.py` / `words.py` | `form_encode` / 单词 PK 报文构造 |
| `errors.py` | 异常层级 |

完整 API 参考见 `src/up366/README.md`。

## 凭据

仓库不含任何真实账号。`login()` 的账号口令由调用方提供；`tools/` 中的脚本（在 `main` 分支）通过 `UP366_USER` / `UP366_PASS` 环境变量读取。
