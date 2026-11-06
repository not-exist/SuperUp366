"""Resolve whether a course job is finished.

``GET/POST /client/student/course/job`` does not reliably carry a completion
flag.  The official H5 homework page fetches
``/client/student/all/course/finish/job/v2`` in parallel and merges
``finishedList`` / ``returnedList`` rows by ``studyTaskId == jobId``.  This
module keeps that merge and the type-specific fallback rules independent of
the UI so they can be tested without a network or a terminal.
"""
from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any

STATE_FINISHED = "finished"
STATE_RETURNED = "returned"
STATE_EXPIRED = "expired"
STATE_UNFINISHED = "unfinished"
STATE_UNKNOWN = "unknown"

LABELS = {
    STATE_FINISHED: "已交",
    STATE_RETURNED: "已退回",
    STATE_EXPIRED: "已过期",
    STATE_UNFINISHED: "未交",
    STATE_UNKNOWN: "状态?",
}


@dataclass(frozen=True)
class JobState:
    state: str
    source: str = "job"
    score: Any = None
    detail: Mapping[str, Any] | None = None

    @property
    def label(self) -> str:
        return LABELS.get(self.state, LABELS[STATE_UNKNOWN])

    @property
    def is_finished(self) -> bool:
        return self.state == STATE_FINISHED


def _as_int(value: Any) -> int | None:
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def _rows_by_task(rows: Any) -> dict[str, dict[str, Any]]:
    result: dict[str, dict[str, Any]] = {}
    if not isinstance(rows, list):
        return result
    for row in rows:
        if not isinstance(row, Mapping):
            continue
        task_id = row.get("studyTaskId")
        if task_id is None or task_id == "":
            continue
        result[str(task_id)] = dict(row)
    return result


def build_status_indexes(
    data: Mapping[str, Any] | None,
) -> tuple[dict[str, dict[str, Any]], dict[str, dict[str, Any]]]:
    """Return ``(finished_by_task_id, returned_by_task_id)``."""
    data = data or {}
    return (
        _rows_by_task(data.get("finishedList")),
        _rows_by_task(data.get("returnedList")),
    )


def parse_deadline(value: Any) -> datetime | None:
    """Parse ``endTime`` into an aware UTC datetime, or return ``None``."""
    if value is None or value == "":
        return None
    if isinstance(value, datetime):
        return (
            value.replace(tzinfo=timezone.utc)
            if value.tzinfo is None
            else value.astimezone(timezone.utc)
        )
    if isinstance(value, (int, float)):
        try:
            return datetime.fromtimestamp(float(value) / 1000, tz=timezone.utc)
        except (OverflowError, OSError, ValueError):
            return None
    text = str(value).strip()
    if not text:
        return None
    if text.isdigit() and len(text) >= 10:
        try:
            return datetime.fromtimestamp(int(text) / 1000, tz=timezone.utc)
        except (OverflowError, OSError, ValueError):
            return None
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def detail_state(detail: Mapping[str, Any] | None) -> str | None:
    """Return the state implied by a ``job/info`` data object."""
    if not detail:
        return None
    task_status = _as_int(detail.get("taskStatus"))
    stu_status = _as_int(detail.get("stuStatus"))
    if task_status == 1 or (stu_status is not None and stu_status > 1):
        return STATE_FINISHED
    if task_status is not None or stu_status is not None:
        return STATE_UNFINISHED
    return None


def _flag_state(job: Any) -> str | None:
    """Apply the official H5 type-specific status rules.

    H5 ``testStatus`` uses ``extParam.taskStatus == 1`` for ``type == 1``
    and ``extParam.stuStatus > 1`` for ``type == 3``.  For other types it
    returns "not applicable" (``-1``), so we only report a definite state
    when one of the flags is explicitly present.
    """
    job_type = _as_int(getattr(job, "type", None))
    task_status = _as_int(getattr(job, "task_status", None))
    stu_status = _as_int(getattr(job, "stu_status", None))

    if job_type == 1:
        if task_status is None:
            return None
        return STATE_FINISHED if task_status == 1 else STATE_UNFINISHED
    if job_type == 3:
        if stu_status is None:
            return None
        return STATE_FINISHED if stu_status > 1 else STATE_UNFINISHED
    if task_status == 1 or (stu_status is not None and stu_status > 1):
        return STATE_FINISHED
    if task_status is not None or stu_status is not None:
        return STATE_UNFINISHED
    return None


def resolve_job_state(
    job: Any,
    *,
    finished: Mapping[str, Mapping[str, Any]] | None = None,
    returned: Mapping[str, Mapping[str, Any]] | None = None,
    detail_status: str | None = None,
    now: datetime | None = None,
) -> JobState:
    """Resolve one job's completion state.

    Priority: returned list > ``job/info`` finished > ``job/info`` unfinished
    or expired > finished list > job-list flags > deadline > type default.
    ``job/info.taskStatus`` is the authoritative completion marker (see the
    read-aloud runbook); a finished-list row alone can exist without the job
    being completed.
    """
    job_id = getattr(job, "job_id", None)
    key = str(job_id) if job_id not in (None, "") else ""
    current = now or datetime.now(timezone.utc)
    if current.tzinfo is None:
        current = current.replace(tzinfo=timezone.utc)
    else:
        current = current.astimezone(timezone.utc)

    if key and returned and key in returned:
        return JobState(
            STATE_RETURNED, "returned_list", detail=returned[key]
        )
    if detail_status == STATE_FINISHED:
        return JobState(
            STATE_FINISHED,
            "job_info",
            detail=(finished or {}).get(key),
        )

    deadline = parse_deadline(getattr(job, "end_time", None))
    expired = deadline is not None and current >= deadline

    if detail_status == STATE_UNFINISHED:
        return JobState(
            STATE_EXPIRED if expired else STATE_UNFINISHED, "job_info"
        )
    if key and finished and key in finished:
        row = finished[key]
        return JobState(
            STATE_FINISHED, "finished_list", score=row.get("score"), detail=row
        )

    flag = _flag_state(job)
    if flag == STATE_FINISHED:
        return JobState(STATE_FINISHED, "job")
    if expired:
        return JobState(STATE_EXPIRED, "deadline")
    if flag == STATE_UNFINISHED:
        return JobState(STATE_UNFINISHED, "job")

    job_type = _as_int(getattr(job, "type", None))
    if job_type in (1, 3):
        return JobState(STATE_UNFINISHED, "job")
    return JobState(STATE_UNKNOWN, "job")
