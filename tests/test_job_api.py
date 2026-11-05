from __future__ import annotations

import json

import httpx
import pytest

import up366.client as client_module
import up366.job_api as job_api_module
from up366 import Up366, Up366APIError
from up366.job import Job, JobStatus

FINISHED_TASKS_URL = "https://dsb-api.up366.cn/client/book/finished-task/list"
ANSWER_DETAIL_URL = "https://dsb-api.up366.cn/client/student/task/answer/detail/v2"
TEST_ALL_DATA_URL = "https://xot-api.up366.cn/client/onlinetesting/alldata"
PAPER_ZIP_INFO_URL = "https://xot-api.up366.cn/client/paper/zipinfo"
PAPER_DETAIL_URL = "https://qlib-api.up366.cn/client/paper/detail"
CHAPTER_QUESTIONS_URL = "https://book-api.up366.cn/client/chapters/list-question-file"
TASK_LINKED_URL = "https://book-api.up366.cn/client/task/linked"
SUBMIT_TASK_SCORE_URL = "https://study-api.up366.cn/client/task/score/submit/v2"
SUBMIT_ONLINE_TEST_URL = "https://xot-api.up366.cn/client/onlinetesting/submit"
JOB_INFO_URL = "https://studytask-api.up366.cn/client/student/job/info"

KNOWN_UT = "faa48af05ea79795663472f42357ba37324a864d9a71e97fade8497f01a2bddf"
KNOWN_TASKS_JSON = (
    '[{"uid":39370562,"studyTaskId":"JOB-1","score":8,'
    '"qstJson":[{"elementId":"E1"}],"extendParams":{"batchId":"B1"}}]'
)


def _response(payload: dict, status_code: int = 200, method: str = "POST", url: str = "https://example.test") -> httpx.Response:
    return httpx.Response(
        status_code,
        json=payload,
        request=httpx.Request(method, url),
    )


def _ok(data) -> dict:
    return {"result": {"code": 0, "msg": "ok"}, "data": data}


def _session() -> Up366:
    return Up366(
        username="alice",
        password_hash="hash",
        tgt="TGT-123",
        client_id="CID-TEST",
        cookies=httpx.Cookies(),
    )


def _capture(monkeypatch, payload: dict) -> dict:
    captured: dict[str, object] = {}

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(
            method=method, url=url, data=data, headers=headers,
            timeout=timeout, cookies=cookies,
        )
        return _response(payload, method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)
    return captured


def _job() -> Job:
    return Job.from_dict(
        {
            "jobId": "JOB-1",
            "jobName": "Unit 4",
            "courseId": 9222783,
            "contentId": "CONTENT-1",
            "extParam": json.dumps({"bookId": "BOOK-1"}),
        }
    )


# --- read endpoints: exact method / url / data -----------------------------

def test_finished_tasks_posts_book_and_course_and_returns_array(monkeypatch):
    rows = [{"taskId": "T-1", "score": 8}]
    captured = _capture(monkeypatch, _ok(rows))
    result = _session().finished_tasks("BOOK-1", 9222783)
    assert result == rows
    assert captured["method"] == "POST"
    assert captured["url"] == FINISHED_TASKS_URL
    assert captured["data"] == {"bookId": "BOOK-1", "courseId": "9222783"}


def test_finished_tasks_omits_course_id_when_not_given(monkeypatch):
    captured = _capture(monkeypatch, _ok([]))
    assert _session().finished_tasks("BOOK-1") == []
    assert captured["data"] == {"bookId": "BOOK-1"}


def test_finished_tasks_returns_empty_when_data_missing(monkeypatch):
    _capture(monkeypatch, {"result": {"code": 0, "msg": "ok"}})
    assert _session().finished_tasks("BOOK-1") == []


def test_answer_detail_posts_params_as_data(monkeypatch):
    info = {"answers": [1, 2]}
    captured = _capture(monkeypatch, _ok(info))
    result = _session().answer_detail({"taskId": "T-1", "uid": 39370562})
    assert result == info
    assert captured["method"] == "POST"
    assert captured["url"] == ANSWER_DETAIL_URL
    assert captured["data"] == {"taskId": "T-1", "uid": "39370562"}


def test_answer_detail_use_v2_false_uses_get_fallback(monkeypatch):
    captured = _capture(monkeypatch, _ok({"ok": True}))
    _session().answer_detail({"taskId": "T-1"}, use_v2=False)
    assert captured["method"] == "GET"
    assert captured["url"] == ANSWER_DETAIL_URL + "?taskId=T-1"
    assert captured["data"] is None


def test_test_all_data_posts_expected_request(monkeypatch):
    info = {"testInfo": {"paperId": "PAPER-1"}}
    captured = _capture(monkeypatch, _ok(info))
    result = _session().test_all_data("TEST-1", 39370562)
    assert result == info
    assert captured["method"] == "POST"
    assert captured["url"] == TEST_ALL_DATA_URL
    assert captured["data"] == {"testId": "TEST-1", "uid": "39370562"}


def test_paper_zip_info_posts_expected_request(monkeypatch):
    info = {"downloadurl": "https://x/y.zip", "fileId": "F-1"}
    captured = _capture(monkeypatch, _ok(info))
    result = _session().paper_zip_info("PAPER-1")
    assert result == info
    assert captured["method"] == "POST"
    assert captured["url"] == PAPER_ZIP_INFO_URL
    assert captured["data"] == {"paperId": "PAPER-1"}


def test_paper_detail_uses_get_params_not_data(monkeypatch):
    xml = '<elements><element id="Q1"/></elements>'
    captured = _capture(monkeypatch, _ok({"paperData": xml}))
    result = _session().paper_detail("PAPER-1")
    assert result == xml
    assert captured["method"] == "GET"
    assert captured["url"] == PAPER_DETAIL_URL + "?paperId=PAPER-1"
    assert captured["data"] is None


def test_paper_detail_returns_empty_when_no_paper_data(monkeypatch):
    _capture(monkeypatch, _ok({}))
    assert _session().paper_detail("PAPER-1") == ""


def test_chapter_questions_posts_expected_request(monkeypatch):
    questions = [{"id": "Q1"}]
    captured = _capture(monkeypatch, _ok({"dirQuestions": questions}))
    result = _session().chapter_questions("B-1", "C-1", "F-1")
    assert result == questions
    assert captured["method"] == "POST"
    assert captured["url"] == CHAPTER_QUESTIONS_URL
    assert captured["data"] == {"bookId": "B-1", "chapterId": "C-1", "fileId": "F-1"}


def test_chapter_questions_returns_empty_when_list_missing(monkeypatch):
    _capture(monkeypatch, _ok({}))
    assert _session().chapter_questions("B-1", "C-1", "F-1") == []


def test_task_linked_posts_expected_request(monkeypatch):
    chapters = [{"id": "C-1", "parentId": "0"}]
    captured = _capture(monkeypatch, _ok({"chapters": chapters}))
    result = _session().task_linked("B-1", "T-1")
    assert result == chapters
    assert captured["method"] == "POST"
    assert captured["url"] == TASK_LINKED_URL
    assert captured["data"] == {"bookId": "B-1", "taskId": "T-1", "type": "1"}


def test_task_linked_passes_type_and_returns_empty_when_missing(monkeypatch):
    captured = _capture(monkeypatch, _ok({}))
    assert _session().task_linked("B-1", "T-1", type=2) == []
    assert captured["data"]["type"] == "2"


def test_job_status_uses_job_info_body_and_wraps(monkeypatch):
    info = {"taskStatus": 1, "submitTime": 1793714840149, "stuStatus": 2}
    captured = _capture(monkeypatch, _ok(info))
    result = _session().job_status(9222783, "JOB-1", "CONTENT-1", "BOOK-1")
    assert isinstance(result, JobStatus)
    assert result.is_finished is True
    assert captured["method"] == "POST"
    assert captured["url"] == JOB_INFO_URL
    assert captured["data"] == {
        "courseId": "9222783",
        "jobId": "JOB-1",
        "studyTaskId": "JOB-1",
        "contentId": "CONTENT-1",
        "bookId": "BOOK-1",
    }


# --- Job / JobStatus models -------------------------------------------------

def test_job_new_properties_from_ext_param_and_is_finished():
    job = Job.from_dict(
        {
            "jobId": "JOB-1",
            "contentId": "CONTENT-1",
            "extParam": json.dumps(
                {
                    "paperId": "PAPER-1",
                    "studyTaskPid": "PID-1",
                    "taskStatus": 1,
                    "stuStatus": 2,
                    "submitTime": 1793714840149,
                }
            ),
        }
    )
    assert job.paper_id == "PAPER-1"
    assert job.study_task_pid == "PID-1"
    assert job.task_id == "CONTENT-1"
    assert job.task_status == 1
    assert job.stu_status == 2
    assert job.submit_time == 1793714840149
    assert job.is_finished is True


def test_job_new_properties_fall_back_to_raw_and_not_finished():
    job = Job.from_dict(
        {
            "contentId": "C-RAW",
            "paperId": "P-RAW",
            "studyTaskPid": "PID-RAW",
            "taskStatus": 0,
            "stuStatus": 0,
            "submitTime": 0,
        }
    )
    assert job.paper_id == "P-RAW"
    assert job.study_task_pid == "PID-RAW"
    assert job.task_id == "C-RAW"
    assert job.task_status == 0
    assert job.is_finished is False


def test_job_is_finished_by_stu_status_above_one():
    assert Job.from_dict({"taskStatus": 0, "stuStatus": 2}).is_finished is True
    assert Job.from_dict({"taskStatus": 0, "stuStatus": 1}).is_finished is False


def test_job_status_fields_and_roundtrip():
    raw = {"taskStatus": 0, "submitTime": 0, "stuStatus": 0}
    status = JobStatus.from_dict(raw)
    assert status.task_status == 0
    assert status.submit_time == 0
    assert status.stu_status == 0
    assert status.is_finished is False
    assert status.raw == raw
    assert status.to_dict() == raw


# --- submit_task_score (branch A: has ut, strict dry_run) ------------------

def _score_row() -> dict:
    return {
        "uid": 39370562,
        "studyTaskId": "JOB-1",
        "score": 8,
        "qstJson": {"E1_1": {"elementId": "E1"}},
        "extendParams": '{"batchId":"B1"}',
    }


def test_submit_task_score_ut_matches_known_vector(monkeypatch):
    monkeypatch.setattr(job_api_module.time, "time", lambda: 1700000000.0)
    calls = []
    monkeypatch.setattr(
        client_module.httpx, "request",
        lambda *a, **k: calls.append(a),
    )
    result = _session().submit_task_score(None, _score_row())
    assert result["tasks_json"] == KNOWN_TASKS_JSON
    assert result["data"]["ut"] == KNOWN_UT
    assert len(result["data"]["ut"]) == 64
    assert calls == []


def test_submit_task_score_dry_run_makes_zero_requests(monkeypatch):
    calls = []
    monkeypatch.setattr(
        client_module.httpx, "request", lambda *a, **k: calls.append(a)
    )
    result = _session().submit_task_score(_job(), _score_row())
    assert calls == []
    assert result["endpoint"] == "submit_task_score"
    assert result["url"] == SUBMIT_TASK_SCORE_URL
    assert result["data"]["submitType"] == "0"
    assert result["data"]["tasksJson"] == result["tasks_json"]


def test_submit_task_score_confirm_false_never_sends(monkeypatch):
    calls = []
    monkeypatch.setattr(
        client_module.httpx, "request", lambda *a, **k: calls.append(a)
    )
    _session().submit_task_score(None, _score_row(), dry_run=False, confirm=False)
    assert calls == []


def test_submit_task_score_study_task_id_mismatch_raises_before_network(monkeypatch):
    calls = []
    monkeypatch.setattr(
        client_module.httpx, "request", lambda *a, **k: calls.append(a)
    )
    row = dict(_score_row(), studyTaskId="WRONG")
    with pytest.raises(ValueError, match="studyTaskId"):
        _session().submit_task_score(_job(), row)
    assert calls == []


def test_submit_task_score_requires_uid(monkeypatch):
    calls = []
    monkeypatch.setattr(
        client_module.httpx, "request", lambda *a, **k: calls.append(a)
    )
    with pytest.raises(ValueError, match="uid"):
        _session().submit_task_score(None, {"studyTaskId": "JOB-1"})
    assert calls == []


def test_submit_task_score_normalizes_qst_json_and_extend_params(monkeypatch):
    result = _session().submit_task_score(None, _score_row())
    row = json.loads(result["tasks_json"])[0]
    assert row["qstJson"] == [{"elementId": "E1"}]
    assert row["extendParams"] == {"batchId": "B1"}
    assert row["uid"] == 39370562
    assert row["studyTaskId"] == "JOB-1"


def test_submit_task_score_confirm_true_sends(monkeypatch):
    captured = _capture(monkeypatch, _ok({"submitted": True}))
    result = _session().submit_task_score(
        None, _score_row(), dry_run=False, confirm=True, verify=False
    )
    assert result == {"data": {"submitted": True}}
    assert captured["method"] == "POST"
    assert captured["url"] == SUBMIT_TASK_SCORE_URL
    assert captured["data"]["submitType"] == "0"
    assert captured["data"]["tasksJson"] == KNOWN_TASKS_JSON
    assert len(captured["data"]["ut"]) == 64


def test_submit_task_score_minus_13_surfaces_without_retry(monkeypatch):
    calls: list[str] = []

    def fake_request(method, url, *, data, headers, timeout, cookies):
        calls.append(url)
        return _response({"result": {"code": -13, "msg": "重复作答"}}, method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)
    with pytest.raises(Up366APIError) as excinfo:
        _session().submit_task_score(
            None, _score_row(), dry_run=False, confirm=True, verify=False
        )
    assert excinfo.value.code == -13
    assert calls == [SUBMIT_TASK_SCORE_URL]


# --- submit_online_test (branch B: no ut) -----------------------------------

def _online_row() -> dict:
    return {
        "taskId": "TEST-1",
        "paperId": "PAPER-1",
        "uid": 39370562,
        "answerData": "<elements/>",
        "duration": 1,
    }


def test_submit_online_test_has_test_id_and_no_ut(monkeypatch):
    calls = []
    monkeypatch.setattr(
        client_module.httpx, "request", lambda *a, **k: calls.append(a)
    )
    result = _session().submit_online_test(_online_row())
    body = result["data"]
    assert body["testId"] == "TEST-1"
    assert body["taskId"] == "TEST-1"
    assert "ut" not in body
    assert result["url"] == SUBMIT_ONLINE_TEST_URL
    assert calls == []


def test_submit_online_test_confirm_true_sends(monkeypatch):
    captured = _capture(monkeypatch, _ok({"ok": True}))
    result = _session().submit_online_test(
        _online_row(), dry_run=False, confirm=True
    )
    assert result == {"ok": True}
    assert captured["method"] == "POST"
    assert captured["url"] == SUBMIT_ONLINE_TEST_URL
    assert captured["data"]["testId"] == "TEST-1"
    assert "ut" not in captured["data"]
