from __future__ import annotations

import httpx
import pytest

import up366.client as client_module
from up366 import Job, Up366, Up366APIError, Up366ResponseError

COURSE_LIST_URL = "https://course-api.up366.cn/client/course/list-of-student/v2"
JOB_LIST_URL = "https://studytask-api.up366.cn/client/student/course/job"
JOB_INFO_URL = "https://studytask-api.up366.cn/client/student/job/info"


def _response(payload: dict, status_code: int = 200, method: str = "POST", url: str = "https://example.test") -> httpx.Response:
    return httpx.Response(
        status_code,
        json=payload,
        request=httpx.Request(method, url),
    )


def _session(cookies=None) -> Up366:
    return Up366(
        username="alice",
        password_hash="hash",
        tgt="TGT-123",
        client_id="CID-TEST",
        cookies=httpx.Cookies(cookies or {}),
    )


def test_courses_posts_expected_request(monkeypatch):
    captured: dict[str, object] = {}
    courses = [
        {"courseId": 9222783, "courseName": "2028届17班", "status": 1},
    ]

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(
            {"result": {"code": 0, "msg": "ok"}, "data": {"list": courses}},
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session(cookies={"CASTGC": "abc"}).courses()

    assert result == courses
    assert captured["method"] == "POST"
    assert captured["url"] == COURSE_LIST_URL
    assert captured["data"] == {"status": "1", "pager.pageSize": "999"}
    assert captured["timeout"] == 20.0
    assert captured["headers"]["u3t"] == "TGT-123"
    assert captured["headers"]["clientId"] == "CID-TEST"
    assert captured["headers"]["user-agent"] == "PC-Up366-Student 6.11.0"
    assert captured["cookies"]["CASTGC"] == "abc"


def test_jobs_posts_expected_request_and_returns_all_task_list(monkeypatch):
    captured: dict[str, object] = {}
    all_tasks = [{"jobId": "JOB-1", "contentId": "CONTENT-1", "jobName": "Unit 4"}]

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(
            {
                "result": {"code": 0, "msg": "ok"},
                "data": {
                    "allTaskList": all_tasks,
                    "ownerTaskList": [{"jobId": "OWNER-1"}],
                },
            },
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().jobs(9222783)

    assert len(result) == 1
    job = result[0]
    assert isinstance(job, Job)
    assert job.job_id == "JOB-1"
    assert job.content_id == "CONTENT-1"
    assert job.name == "Unit 4"
    assert job.raw == all_tasks[0]
    assert captured["method"] == "POST"
    assert captured["url"] == JOB_LIST_URL
    assert captured["data"] == {
        "courseId": "9222783",
        "pageIdFlag": "1",
        "version": "",
    }


def test_job_info_posts_expected_request_and_returns_data(monkeypatch):
    captured: dict[str, object] = {}
    info = {
        "jobId": "JOB-1",
        "contentId": "CONTENT-1",
        "bookId": "BOOK-1",
        "paperId": "PAPER-1",
        "taskStatus": 1,
    }

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(
            {"result": {"code": 0, "msg": "ok"}, "data": info},
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().job_info(9222783, "JOB-1", "CONTENT-1", "BOOK-1")

    assert result == info
    assert captured["method"] == "POST"
    assert captured["url"] == JOB_INFO_URL
    assert captured["data"] == {
        "courseId": "9222783",
        "jobId": "JOB-1",
        "studyTaskId": "JOB-1",
        "contentId": "CONTENT-1",
        "bookId": "BOOK-1",
    }


def test_request_retries_once_with_service_ticket(monkeypatch):
    calls: list[str] = []

    def fake_request(method, url, *, data, headers, timeout, cookies):
        calls.append(url)
        if len(calls) == 1:
            return _response(
                {"result": {"code": -21, "msg": "没有登录"}},
                method=method,
                url=url,
            )
        return _response(
            {"result": {"code": 0, "msg": "ok"}, "data": {"allTaskList": []}},
            method=method,
            url=url,
        )

    def fake_post(url, *, data, headers, timeout, cookies):
        assert cookies["CASTGC"] == "abc"
        assert url == "https://sso.up366.cn/v1/serviceTicket"
        assert data == {"service": JOB_LIST_URL}
        return _response(
            {"result": {"code": -24, "msg": "ok"}, "data": {"serverTicket": "ST-123"}},
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)
    monkeypatch.setattr(client_module.httpx, "post", fake_post)

    result = _session(cookies={"CASTGC": "abc"}).jobs(9222783)

    assert result == []
    assert calls == [JOB_LIST_URL, JOB_LIST_URL + "?ticket=ST-123"]


def test_request_raises_api_error_for_business_failure(monkeypatch):
    def fake_request(method, url, *, data, headers, timeout, cookies):
        return _response(
            {"result": {"code": -4, "msg": "无权限"}},
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    with pytest.raises(Up366APIError) as excinfo:
        _session().jobs(9222783)

    assert excinfo.value.code == -4
    assert "无权限" in str(excinfo.value)


def test_courses_rejects_malformed_list(monkeypatch):
    def fake_request(method, url, *, data, headers, timeout, cookies):
        return _response(
            {"result": {"code": 0, "msg": "ok"}, "data": {"list": "not-a-list"}},
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    with pytest.raises(Up366ResponseError):
        _session().courses()
