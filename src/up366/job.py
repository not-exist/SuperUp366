from __future__ import annotations

import json
from collections.abc import Mapping
from typing import Any

__all__ = ["Job", "JobStatus"]


def _parse_ext_param(value: Any) -> dict[str, Any]:
    if isinstance(value, Mapping):
        return dict(value)
    if isinstance(value, str):
        try:
            parsed = json.loads(value)
        except ValueError:
            return {}
        return parsed if isinstance(parsed, dict) else {}
    return {}


def _as_number(value: Any) -> float | int | None:
    """Coerce the server's status value, which may be a number or a string."""
    if isinstance(value, bool):
        return int(value)
    if isinstance(value, (int, float)):
        return value
    if isinstance(value, str):
        text = value.strip()
        if not text:
            return None
        try:
            return float(text) if "." in text else int(text)
        except ValueError:
            return None
    return None


def _finished(task_status: Any, stu_status: Any) -> bool:
    if _as_number(task_status) == 1:
        return True
    stu = _as_number(stu_status)
    return stu is not None and stu > 1


class Job:
    """A homework/task entry returned by ``Up366.jobs()``."""

    def __init__(self, raw: Mapping[str, Any]):
        if not isinstance(raw, Mapping):
            raise TypeError("Job raw data must be a mapping")
        self.raw = dict(raw)
        self.ext_param = _parse_ext_param(self.raw.get("extParam"))

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "Job":
        return cls(raw)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)

    @property
    def job_id(self) -> str | None:
        return self.raw.get("jobId") or self.raw.get("studyTaskId")

    @property
    def study_task_id(self) -> str | None:
        """The id used as ``studyTaskId`` when submitting results."""
        return self.job_id

    @property
    def name(self) -> str:
        return self.raw.get("jobName") or self.raw.get("name") or ""

    @property
    def course_id(self) -> int | str | None:
        value = self.raw.get("courseId")
        return value if value is not None else self.ext_param.get("courseId")

    @property
    def content_id(self) -> str | None:
        return self.raw.get("contentId")

    @property
    def book_id(self) -> str | None:
        value = self.ext_param.get("bookId")
        return value if value is not None else self.raw.get("bookId")

    @property
    def chapter_id(self) -> str | None:
        value = self.ext_param.get("chapterId")
        return value if value is not None else self.raw.get("chapterId")

    @property
    def task_score(self) -> Any:
        value = self.ext_param.get("taskScore")
        return value if value is not None else self.raw.get("taskScore")

    @property
    def xot_param(self) -> dict[str, Any]:
        return _parse_ext_param(self.ext_param.get("xotParam"))

    @property
    def is_word_pk(self) -> bool:
        """Whether this job is a word-PK task."""
        xot = self.xot_param
        return bool(xot.get("pkBucketId")) or "wordpkPlayType" in xot

    @property
    def type(self) -> Any:
        return self.raw.get("type")

    @property
    def begin_time(self) -> Any:
        return self.raw.get("beginTime")

    @property
    def end_time(self) -> Any:
        return self.raw.get("endTime")

    @property
    def paper_id(self) -> str | None:
        value = self.ext_param.get("paperId")
        return value if value is not None else self.raw.get("paperId")

    @property
    def study_task_pid(self) -> str | None:
        value = self.ext_param.get("studyTaskPid")
        return value if value is not None else self.raw.get("studyTaskPid")

    @property
    def task_id(self) -> str | None:
        """Content/page id used as ``taskId`` (``= contentId``)."""
        value = self.ext_param.get("taskId")
        return value if value is not None else self.raw.get("contentId")

    @property
    def task_status(self) -> Any:
        value = self.ext_param.get("taskStatus")
        return value if value is not None else self.raw.get("taskStatus")

    @property
    def stu_status(self) -> Any:
        value = self.ext_param.get("stuStatus")
        return value if value is not None else self.raw.get("stuStatus")

    @property
    def submit_time(self) -> Any:
        value = self.ext_param.get("submitTime")
        return value if value is not None else self.raw.get("submitTime")

    @property
    def is_finished(self) -> bool:
        return _finished(self.task_status, self.stu_status)

    def __repr__(self) -> str:
        return f"Job(job_id={self.job_id!r}, name={self.name!r})"


class JobStatus:
    """Server-side completion status of one job, from ``job/info`` data."""

    def __init__(self, raw: Mapping[str, Any]):
        if not isinstance(raw, Mapping):
            raise TypeError("JobStatus raw data must be a mapping")
        self.raw = dict(raw)

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "JobStatus":
        return cls(raw)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)

    @property
    def task_status(self) -> Any:
        return self.raw.get("taskStatus")

    @property
    def submit_time(self) -> Any:
        return self.raw.get("submitTime")

    @property
    def stu_status(self) -> Any:
        return self.raw.get("stuStatus")

    @property
    def is_finished(self) -> bool:
        return _finished(self.task_status, self.stu_status)

    def __repr__(self) -> str:
        return (
            f"JobStatus(task_status={self.task_status!r}, "
            f"is_finished={self.is_finished!r})"
        )
