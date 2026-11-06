"""Read-aloud (跟读) preparation with a deliberately empty submit step.

The wire format for submitting an arbitrary read-aloud job is still under
investigation, and the user asked to leave the homework-submit function
blank.  This module therefore only:

* maps one mp3 per read-aloud question,
* writes a local JSON plan, and
* exposes a submit stub that never touches the network.

No audio is uploaded and no score row is posted here.
"""
from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any

from .paper import Paper, Question

DEFAULT_PLAN_DIR = Path.home() / ".up366-tui" / "readaloud-plans"


@dataclass
class ReadAloudPlan:
    job_id: str
    job_name: str
    course_id: str
    content_id: str
    book_id: str
    paper_id: str
    source: str
    created_at: str
    questions: list[dict[str, Any]] = field(default_factory=list)
    mp3_files: list[dict[str, Any]] = field(default_factory=list)
    submission: str = "not_implemented"

    def to_dict(self) -> dict[str, Any]:
        return {
            "job_id": self.job_id,
            "job_name": self.job_name,
            "course_id": self.course_id,
            "content_id": self.content_id,
            "book_id": self.book_id,
            "paper_id": self.paper_id,
            "source": self.source,
            "created_at": self.created_at,
            "questions": self.questions,
            "mp3_files": self.mp3_files,
            "submission": self.submission,
        }


def _created_at() -> str:
    return datetime.now().astimezone().isoformat(timespec="seconds")


def _job_field(job: Any, name: str) -> str:
    value = getattr(job, name, "")
    return "" if value is None else str(value)


def _file_record(index: int, path: Path, question: Question | None) -> dict[str, Any]:
    path = path.expanduser()
    if not path.is_file():
        raise ValueError(f"文件不存在或不是普通文件：{path}")
    if path.suffix.lower() != ".mp3":
        raise ValueError(f"需要 .mp3 文件：{path}")
    return {
        "index": index,
        "path": str(path.resolve()),
        "name": path.name,
        "size": path.stat().st_size,
        "question_element_id": question.element_id if question is not None else "",
    }


def _question_record(index: int, question: Question) -> dict[str, Any]:
    return {
        "index": index,
        "element_id": question.element_id,
        "question_no": question.question_no,
        "score": question.score,
        "duration": question.duration,
        "sentence_count": question.sentence_count,
        "preview": question.preview[:200],
    }


def build_plan(
    *,
    job: Any,
    paper: Paper,
    paths: list[Path],
    course_id: Any = None,
    paper_id: Any = None,
) -> ReadAloudPlan:
    """Build a plan from the paper's read-aloud questions and user files."""
    questions = paper.readaloud_questions
    if not questions:
        raise ValueError("试卷中没有跟读题（question_type=9）")
    if len(paths) != len(questions):
        raise ValueError(
            f"需要 {len(questions)} 段 mp3，实际给了 {len(paths)} 段"
        )
    plan = ReadAloudPlan(
        job_id=_job_field(job, "job_id"),
        job_name=_job_field(job, "name"),
        course_id=(
            str(course_id)
            if course_id is not None
            else _job_field(job, "course_id")
        ),
        content_id=_job_field(job, "content_id"),
        book_id=_job_field(job, "book_id"),
        paper_id=(
            str(paper_id)
            if paper_id is not None
            else _job_field(job, "paper_id")
        ),
        source="paper",
        created_at=_created_at(),
    )
    for index, (question, path) in enumerate(zip(questions, paths), 1):
        plan.questions.append(_question_record(index, question))
        plan.mp3_files.append(_file_record(index, path, question))
    return plan


def build_manual_plan(
    job: Any,
    paths: list[Path],
    *,
    course_id: Any = None,
    paper_id: Any = "",
) -> ReadAloudPlan:
    """Build a plan when the paper structure could not be fetched."""
    if not paths:
        raise ValueError("至少需要一段 mp3")
    plan = ReadAloudPlan(
        job_id=_job_field(job, "job_id"),
        job_name=_job_field(job, "name"),
        course_id=(
            str(course_id)
            if course_id is not None
            else _job_field(job, "course_id")
        ),
        content_id=_job_field(job, "content_id"),
        book_id=_job_field(job, "book_id"),
        paper_id=str(paper_id or ""),
        source="manual",
        created_at=_created_at(),
    )
    for index, path in enumerate(paths, 1):
        plan.questions.append(
            {
                "index": index,
                "element_id": "",
                "question_no": str(index),
                "score": None,
                "duration": None,
                "sentence_count": 0,
                "preview": "",
            }
        )
        plan.mp3_files.append(_file_record(index, path, None))
    return plan


def save_plan(
    plan: ReadAloudPlan,
    *,
    root: Path | str | None = None,
) -> Path:
    """Write the plan to a local JSON file and return its path."""
    directory = Path(root) if root is not None else DEFAULT_PLAN_DIR
    directory.mkdir(parents=True, exist_ok=True)
    safe_job = re.sub(r"[^A-Za-z0-9_-]+", "_", plan.job_id or "job")[:60]
    stamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    path = directory / f"readaloud-{safe_job}-{stamp}.json"
    suffix = 1
    while path.exists():
        path = directory / f"readaloud-{safe_job}-{stamp}-{suffix}.json"
        suffix += 1
    path.write_text(
        json.dumps(plan.to_dict(), ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return path


def submit_plan(session: Any, plan: ReadAloudPlan) -> None:
    """Intentionally empty: the remote submit format is not settled yet.

    Keeping this as an explicit stub (rather than silently skipping it in the
    caller) makes the unfinished part of the feature visible and testable.
    """
    del session, plan
    raise NotImplementedError(
        "跟读作业提交功能留空：提交报文格式尚未确定；"
        "未上传音频，也未发送任何提交请求。"
    )
