"""Ordinary job API mixin (read + write endpoints for homework tasks)."""
from __future__ import annotations

import hashlib
import json
import time
from collections.abc import Mapping
from typing import Any

from .endpoints import ENDPOINTS
from .errors import Up366ResponseError
from .job import JobStatus

_SUBMIT_TOKEN_SALT = "submitTaskToken-pc-987654"


def _build_ut(tasks_json: str) -> str:
    """Return the 64-char submit signature for ``tasksJson`` (branch A)."""
    ts = hashlib.md5(str(int(time.time() * 1000)).encode()).hexdigest()
    middle = hashlib.md5(
        (
            hashlib.md5(tasks_json.encode()).hexdigest()
            + _SUBMIT_TOKEN_SALT
            + ts
        ).encode()
    ).hexdigest()
    return ts[:10] + middle + ts[10:]


def _normalize_row(row: Mapping[str, Any]) -> dict[str, Any]:
    """Apply the client's pre-submit rewrites to one ``TaskStuded`` row."""
    normalized: dict[str, Any] = {}
    for key, value in row.items():
        if key == "qstJson" and isinstance(value, dict):
            normalized[key] = list(value.values())
        elif key == "extendParams" and isinstance(value, str):
            try:
                parsed = json.loads(value) if value.strip() else {}
            except ValueError:
                parsed = {}
            normalized[key] = parsed if isinstance(parsed, dict) else {}
        else:
            normalized[key] = value
    return normalized


class JobAPIMixin:
    def finished_tasks(self, book_id) -> list[dict]:
        """Return finished-task scores for one book.

        Both official clients send only ``bookId``: the PC client uses GET,
        the H5 client uses POST. The server routes both methods; this library
        follows the H5 POST.
        """
        endpoint = ENDPOINTS["finished_tasks"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"bookId": book_id},
        )
        if payload.get("data") is None:
            return []
        return self._array_data(payload)

    def answer_detail(self, params: Mapping, *, use_v2: bool = True) -> dict:
        """Return a task's paper/answer/student-answer XML.

        Both variants take the same four fields:
        ``courseId, uid, bookId, taskId`` (the H5 mock-exam route inherits
        ``courseId`` from its parent route; omitting it makes v2 fail with
        ``param_missing``). ``use_v2=True`` sends POST
        ``/client/student/task/answer/detail/v2``; ``use_v2=False`` sends GET
        ``/client/student/task/answer/detail`` with query parameters.
        Both return ``data.paperInfo`` plus ``data.userAnswer``.
        """
        if use_v2:
            endpoint = ENDPOINTS["answer_detail"]
            payload = self._request(
                endpoint.method,
                endpoint.base_url + endpoint.path,
                data=dict(params),
            )
        else:
            endpoint = ENDPOINTS["answer_detail_v1"]
            payload = self._request(
                endpoint.method,
                endpoint.base_url + endpoint.path,
                params=dict(params),
            )
        return self._dict_data(payload)

    def test_all_data(self, test_id, uid) -> dict:
        endpoint = ENDPOINTS["test_all_data"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"testId": test_id, "uid": uid},
        )
        return self._dict_data(payload)

    def paper_zip_info(self, paper_id) -> dict:
        endpoint = ENDPOINTS["paper_zip_info"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"paperId": paper_id},
        )
        return self._dict_data(payload)

    def paper_detail(self, paper_id) -> str:
        endpoint = ENDPOINTS["paper_detail"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            params={"paperId": paper_id},
        )
        data = payload.get("data")
        if not isinstance(data, dict):
            return ""
        return data.get("paperData") or ""

    def chapter_questions(self, book_id, chapter_id, file_id) -> list[dict]:
        endpoint = ENDPOINTS["chapter_questions"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"bookId": book_id, "chapterId": chapter_id, "fileId": file_id},
        )
        data = payload.get("data")
        if not isinstance(data, dict):
            return []
        return data.get("dirQuestions") or []

    def task_linked(self, book_id, task_id, *, type: int = 1) -> list[dict]:
        endpoint = ENDPOINTS["task_linked"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"bookId": book_id, "taskId": task_id, "type": type},
        )
        data = payload.get("data")
        if not isinstance(data, dict):
            return []
        return data.get("chapters") or []

    def submit_task_score(
        self,
        job,
        row: Mapping[str, Any],
        *,
        submit_type: str = "0",
        dry_run: bool = True,
        confirm: bool = False,
        verify: bool = True,
    ) -> dict:
        if not isinstance(row, Mapping):
            raise ValueError("row must be a mapping")
        if "uid" not in row:
            raise ValueError("row must contain 'uid'")
        if job is not None and row.get("studyTaskId") != job.job_id:
            raise ValueError("row['studyTaskId'] must equal job.job_id")

        normalized = _normalize_row(row)
        tasks_json = json.dumps(
            [normalized], separators=(",", ":"), ensure_ascii=False
        )
        endpoint = ENDPOINTS["submit_task_score"]
        url = endpoint.base_url + endpoint.path
        body = {
            "tasksJson": tasks_json,
            "submitType": submit_type,
            "ut": _build_ut(tasks_json),
        }

        if dry_run or not confirm:
            return {
                "endpoint": "submit_task_score",
                "url": url,
                "data": body,
                "tasks_json": tasks_json,
            }

        payload = self._request(endpoint.method, url, data=body)
        data = payload.get("data")
        result: dict[str, Any] = {
            "data": data if isinstance(data, (dict, list)) else {}
        }
        if verify and job is not None:
            result["status"] = self.job_status(
                job.course_id, job.job_id, job.content_id, job.book_id
            )
        return result

    def submit_online_test(
        self,
        row: Mapping[str, Any],
        *,
        dry_run: bool = True,
        confirm: bool = False,
    ) -> dict:
        if not isinstance(row, Mapping):
            raise ValueError("row must be a mapping")
        body = dict(row)
        if "taskId" not in body:
            raise ValueError("row must contain 'taskId'")
        body["testId"] = body["taskId"]
        endpoint = ENDPOINTS["submit_online_test"]
        url = endpoint.base_url + endpoint.path

        if dry_run or not confirm:
            return {
                "endpoint": "submit_online_test",
                "url": url,
                "data": body,
            }

        payload = self._request(endpoint.method, url, data=body)
        data = payload.get("data")
        return data if isinstance(data, (dict, list)) else {}

    def job_status(self, course_id, job_id, content_id, book_id) -> JobStatus:
        endpoint = ENDPOINTS["job_info"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "courseId": course_id,
                "jobId": job_id,
                "studyTaskId": job_id,
                "contentId": content_id,
                "bookId": book_id,
            },
        )
        data = payload.get("data")
        if not isinstance(data, dict):
            raise Up366ResponseError(
                "job/info response did not contain a data object"
            )
        return JobStatus.from_dict(data)
