from __future__ import annotations

import httpx
import pytest

import up366.client as client_module
from up366 import Up366, Up366ResponseError
from up366.wrongnote import (
    WrongBook,
    WrongCourse,
    WrongKnowledge,
    WrongNotePage,
    WrongQuestion,
    WrongRank,
    WrongSubject,
    WrongSubjectStat,
)

BASE = "https://wrongnote-api.up366.cn"
URLS = {
    "wrong_subjects": BASE + "/client/wrongnote/student/subject/list",
    "wrong_subject_stat": BASE + "/client/wrongnote/student/subject/stat",
    "wrong_books": BASE + "/client/wrongnote/student/book/list",
    "wrong_knowledge": BASE + "/client/wrongnote/student/knowledge/list/v3",
    "wrong_courses": BASE + "/client/wrongnote/student/course/list",
    "wrong_rank": BASE + "/client/wrongnote/v2/rank/list",
    "wrong_killed_questions": BASE + "/client/wrongnote/kill/question/show",
    "wrong_question_show": BASE + "/client/wrongnote/student/question/show",
    "wrong_kill_paper": BASE + "/client/wrongnote/student/questions/kill",
    "submit_wrong_question": BASE + "/client/wrongnote/student/question/submit",
}


def _response(
    payload: dict,
    status_code: int = 200,
    method: str = "POST",
    url: str = "https://example.test",
) -> httpx.Response:
    return httpx.Response(
        status_code,
        json=payload,
        request=httpx.Request(method, url),
    )


def _session() -> Up366:
    return Up366(
        username="alice",
        password_hash="hash",
        tgt="TGT-123",
        client_id="CID-TEST",
        cookies=httpx.Cookies(),
    )


def _install(monkeypatch, payload, captured=None, calls=None):
    def fake_request(method, url, *, data, headers, timeout, cookies):
        if calls is not None:
            calls.append((method, url, data))
        if captured is not None:
            captured.update(
                method=method,
                url=url,
                data=data,
                headers=headers,
                timeout=timeout,
                cookies=cookies,
            )
        return _response(payload, method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)


def _ok(data):
    return {"result": {"code": 0, "msg": "ok"}, "data": data}


# --- reads ---------------------------------------------------------------


def test_wrong_subjects_posts_and_returns_models(monkeypatch):
    captured = {}
    subjects = [
        {
            "id": 11,
            "subjectName": "数学",
            "subjectId": 2,
            "stageId": 3,
            "wrongCount": 8,
            "killedCount": 5,
            "killedCountLately": 2,
        },
    ]
    _install(monkeypatch, _ok(subjects), captured=captured)

    result = _session().wrong_subjects()

    assert captured["method"] == "POST"
    assert captured["url"] == URLS["wrong_subjects"]
    assert captured["data"] is None
    assert len(result) == 1
    subject = result[0]
    assert isinstance(subject, WrongSubject)
    assert subject.id == 11
    assert subject.subject_name == "数学"
    assert subject.subject_id == 2
    assert subject.stage_id == 3
    assert subject.wrong_count == 8
    assert subject.killed_count == 5
    assert subject.killed_count_lately == 2
    assert subject.to_dict() == subjects[0]


def test_wrong_subject_stat_posts_and_returns_model(monkeypatch):
    captured = {}
    stat = {"wrongCount": 8, "killedCount": 5, "killedCountLately": 2}
    _install(monkeypatch, _ok(stat), captured=captured)

    result = _session().wrong_subject_stat(
        stage_id=3, subject_id=2, start_time="2026-01-01", end_time="2026-01-31"
    )

    assert captured["method"] == "POST"
    assert captured["url"] == URLS["wrong_subject_stat"]
    assert captured["data"] == {
        "bookId": "0",
        "stageId": "3",
        "subjectId": "2",
        "startTime": "2026-01-01",
        "endTime": "2026-01-31",
    }
    assert isinstance(result, WrongSubjectStat)
    assert result.wrong_count == 8
    assert result.killed_count == 5
    assert result.killed_count_lately == 2


def test_wrong_books_adds_synthetic_entries_without_mutating_raw(monkeypatch):
    captured = {}
    books = [
        {"bookId": "1", "bookName": "必修一"},
        {"bookId": "2", "bookName": "必修二"},
    ]
    original = list(books)
    _install(monkeypatch, _ok(books), captured=captured)

    result = _session().wrong_books(stage_id=3, subject_id=2)

    assert captured["method"] == "POST"
    assert captured["url"] == URLS["wrong_books"]
    assert captured["data"] == {"stageId": "3", "subjectId": "2"}
    assert [b.book_id for b in result] == ["0", "1", "2", "WORK"]
    assert [b.book_name for b in result] == ["全部", "必修一", "必修二", "练习"]
    assert all(isinstance(b, WrongBook) for b in result)
    assert result[0].is_all is True and result[0].is_practice is False
    assert result[-1].is_practice is True and result[-1].is_all is False
    assert result[1].is_all is False and result[1].is_practice is False
    assert books == original  # raw payload list untouched


def test_wrong_knowledge_returns_typed_page(monkeypatch):
    captured = {}
    rows = [
        {
            "qTypeId": 1,
            "qTypeName": "选择题",
            "knowledgeId": "K1",
            "knowledgeName": "函数",
            "wrongTimes": 9,
            "wrongCount": 4,
        },
    ]
    data = {"list": rows, "pager": {"currentPage": 1, "pageSize": 100, "pageCount": 3}}
    _install(monkeypatch, _ok(data), captured=captured)

    result = _session().wrong_knowledge(
        stage_id=3, subject_id=2, start_time="2026-01-01", end_time="2026-01-31"
    )

    assert captured["method"] == "POST"
    assert captured["url"] == (
        URLS["wrong_knowledge"] + "?pager.currentPage=1&pager.pageSize=100"
    )
    assert captured["data"] == {
        "bookId": "0",
        "stageId": "3",
        "subjectId": "2",
        "startTime": "2026-01-01",
        "endTime": "2026-01-31",
        "orderType": "desc",
        "orderField": "wrongTimes",
        "pager.pageSize": "100",
        "pager.currentPage": "1",
    }
    assert isinstance(result, WrongNotePage)
    assert result.current_page == 1
    assert result.page_size == 100
    assert result.page_count == 3
    assert result.has_next is True
    assert len(result.items) == 1
    knowledge = result.items[0]
    assert isinstance(knowledge, WrongKnowledge)
    assert knowledge.qtype_id == 1
    assert knowledge.qtype_name == "选择题"
    assert knowledge.knowledge_id == "K1"
    assert knowledge.knowledge_name == "函数"
    assert knowledge.wrong_times == 9
    assert knowledge.wrong_count == 4
    assert result.to_dict() == data


def test_wrong_courses_returns_models(monkeypatch):
    captured = {}
    courses = [{"courseId": 922, "courseName": "高三17班"}]
    _install(monkeypatch, _ok(courses), captured=captured)

    result = _session().wrong_courses(stage_id=3, subject_id=2)

    assert captured["method"] == "POST"
    assert captured["url"] == URLS["wrong_courses"]
    assert captured["data"] == {"stageId": "3", "subjectId": "2"}
    assert len(result) == 1
    course = result[0]
    assert isinstance(course, WrongCourse)
    assert course.course_id == 922
    assert course.course_name == "高三17班"


def test_wrong_rank_returns_typed_page_and_no_next(monkeypatch):
    captured = {}
    rows = [
        {
            "wrongCount": 7,
            "killedCount": 3,
            "recentKilledCount": 1,
            "realname": "张三",
        }
    ]
    data = {"list": rows, "pager": {"currentPage": 2, "pageSize": 100, "pageCount": 2}}
    _install(monkeypatch, _ok(data), captured=captured)

    result = _session().wrong_rank(course_id=922)

    assert captured["method"] == "POST"
    assert captured["url"] == (
        URLS["wrong_rank"] + "?pager.currentPage=1&pager.pageSize=100"
    )
    assert captured["data"] == {
        "courseId": "922",
        "orderType": "desc",
        "orderField": "wrongCount",
        "pager.pageSize": "100",
        "pager.currentPage": "1",
    }
    assert isinstance(result, WrongNotePage)
    assert result.page_count == 2
    assert result.has_next is False
    rank = result.items[0]
    assert isinstance(rank, WrongRank)
    assert rank.wrong_count == 7
    assert rank.killed_count == 3
    assert rank.recent_killed_count == 1
    assert rank.realname == "张三"


def test_wrong_killed_questions_builds_expected_request(monkeypatch):
    captured = {}
    rows = [{"questionId": "Q1"}]
    data = {"list": rows, "pager": {"currentPage": 1, "pageSize": 10, "pageCount": 1}}
    _install(monkeypatch, _ok(data), captured=captured)

    result = _session().wrong_killed_questions(
        book_id="1",
        uid=42,
        stage_id=3,
        subject_id=2,
        kill_flag=0,
        start_time="2026-01-01",
        end_time="2026-01-31",
    )

    assert captured["method"] == "POST"
    assert captured["url"] == (
        URLS["wrong_killed_questions"] + "?pager.currentPage=1&pager.pageSize=10"
    )
    assert captured["data"] == {
        "bookId": "1",
        "uid": "42",
        "stageId": "3",
        "subjectId": "2",
        "killFlag": "0",
        "pager.pageSize": "10",
        "pager.currentPage": "1",
        "startTime": "2026-01-01",
        "endTime": "2026-01-31",
    }
    assert isinstance(result, WrongNotePage)
    assert isinstance(result.items[0], WrongQuestion)
    assert result.has_next is False


def test_wrong_killed_questions_today_omits_time_range(monkeypatch):
    captured = {}
    _install(monkeypatch, _ok({"list": [], "pager": {}}), captured=captured)

    _session().wrong_killed_questions(
        book_id="1", uid=42, stage_id=3, subject_id=2, kill_flag=1
    )

    assert "startTime" not in captured["data"]
    assert "endTime" not in captured["data"]


def test_wrong_show_empty_data_returns_empty_page(monkeypatch):
    for empty in ({}, []):
        _install(monkeypatch, _ok(empty))

        result = _session().wrong_killed_questions(
            book_id="1", uid=42, stage_id=3, subject_id=2, kill_flag=1
        )

        assert result.items == []
        assert result.has_next is False


def test_wrong_question_show_builds_expected_request(monkeypatch):
    captured = {}
    rows = [{"questionId": "Q2", "questionText": "题干"}]
    data = {
        "list": rows,
        "pager": {"currentPage": 1, "pageSize": 10, "pageCount": 1},
        "paperData": "<paper/>",
        "pathPrefix": "cdn/",
    }
    _install(monkeypatch, _ok(data), captured=captured)

    result = _session().wrong_question_show(
        stage_id=3,
        subject_id=2,
        book_id="1",
        qtype_id=5,
        knowledge_id="K1",
        start_time="2026-01-01",
        end_time="2026-01-31",
    )

    assert captured["method"] == "POST"
    assert captured["url"] == (
        URLS["wrong_question_show"] + "?pager.currentPage=1&pager.pageSize=10"
    )
    assert captured["data"] == {
        "stageId": "3",
        "subjectId": "2",
        "bookId": "1",
        "qtypeId": "5",
        "knowledgeId": "K1",
        "startTime": "2026-01-01",
        "endTime": "2026-01-31",
        "pager.pageSize": "10",
        "pager.currentPage": "1",
    }
    assert isinstance(result.items[0], WrongQuestion)
    assert result.raw["paperData"] == "<paper/>"
    assert result.raw["pathPrefix"] == "cdn/"


# --- malformed responses -------------------------------------------------


def test_page_endpoint_rejects_malformed_list(monkeypatch):
    _install(monkeypatch, _ok({"list": "not-a-list", "pager": {}}))
    with pytest.raises(Up366ResponseError):
        _session().wrong_knowledge(stage_id=3, subject_id=2)


def test_page_endpoint_rejects_missing_pager(monkeypatch):
    _install(monkeypatch, _ok({"list": []}))
    with pytest.raises(Up366ResponseError):
        _session().wrong_knowledge(stage_id=3, subject_id=2)


def test_array_endpoint_rejects_dict_data(monkeypatch):
    _install(monkeypatch, _ok({}))
    with pytest.raises(Up366ResponseError):
        _session().wrong_subjects()


# --- reads: kill paper ----------------------------------------------------


def test_wrong_kill_paper_sends_expected_request(monkeypatch):
    captured = {}
    paper = {
        "pathPrefix": "https://cdn/",
        "paperData": "<paper/>",
        "answerData": "<answer/>",
    }
    _install(monkeypatch, _ok(paper), captured=captured)

    result = _session().wrong_kill_paper(
        book_id="1",
        stage_id=3,
        subject_id=2,
        start_time="2026-01-01",
        end_time="2026-01-31",
    )

    assert result == paper
    assert captured["method"] == "POST"
    assert captured["url"] == URLS["wrong_kill_paper"]
    assert captured["data"] == {
        "bookId": "1",
        "stageId": "3",
        "subjectId": "2",
        "startTime": "2026-01-01",
        "endTime": "2026-01-31",
    }


def test_wrong_kill_paper_empty_data_returns_empty_dict(monkeypatch):
    for empty in ({}, []):
        _install(monkeypatch, _ok(empty))

        result = _session().wrong_kill_paper(book_id="1", stage_id=3, subject_id=2)

        assert result == {}


# --- writes (strict dry_run, never real) ---------------------------------


def test_submit_wrong_question_dry_run_default_no_network(monkeypatch):
    calls = []
    _install(monkeypatch, _ok({}), calls=calls)

    result = _session().submit_wrong_question({"uid": "1", "result": "{}"})

    assert calls == []
    assert result == {
        "endpoint": "submit_wrong_question",
        "url": URLS["submit_wrong_question"],
        "data": {"uid": "1", "result": "{}"},
    }


def test_submit_wrong_question_confirm_true_sends(monkeypatch):
    captured = {}
    response_payload = _ok({"ok": True})
    _install(monkeypatch, response_payload, captured=captured)

    result = _session().submit_wrong_question(
        {"uid": "1", "result": "{}"}, dry_run=False, confirm=True
    )

    assert captured["method"] == "POST"
    assert captured["url"] == URLS["submit_wrong_question"]
    assert captured["data"] == {"uid": "1", "result": "{}"}
    assert result == response_payload


def test_write_rejects_empty_payload(monkeypatch):
    calls = []
    _install(monkeypatch, _ok({}), calls=calls)
    with pytest.raises(ValueError):
        _session().submit_wrong_question({}, dry_run=False, confirm=True)
    assert calls == []


def test_write_rejects_non_mapping_payload(monkeypatch):
    calls = []
    _install(monkeypatch, _ok({}), calls=calls)
    with pytest.raises(ValueError):
        _session().submit_wrong_question(["Q1"], dry_run=False, confirm=True)
    assert calls == []
