from __future__ import annotations

import json
import urllib.parse
from pathlib import Path

from up366 import Job
from up366_tui import app as app_module
from up366_tui.app import App
from up366_tui.jobstatus import (
    STATE_FINISHED,
    STATE_RETURNED,
    STATE_UNFINISHED,
)
from up366_tui.ui import UI

ATTACHMENT = (
    "<root><paragraph>"
    '<sentences id="1"><content><![CDATA[Hello world.]]></content></sentences>'
    "</paragraph></root>"
)


def _word(index: int) -> dict:
    return {
        "entryId": index,
        "entry": f"word{index}",
        "paraphrase": f"释义{index}",
    }


def _job(*, word_pk: bool) -> Job:
    xot = {"pkBucketId": "BUCKET-1"} if word_pk else {"reportType": 0}
    return Job.from_dict(
        {
            "jobId": "JOB-1",
            "jobName": "Unit 4 单词PK" if word_pk else "Unit 4 逐句跟读",
            "courseId": 1,
            "contentId": "CONTENT-1",
            "extParam": json.dumps(
                {
                    "bookId": "BOOK-1",
                    "paperId": "PAPER-1",
                    "xotParam": json.dumps(xot),
                }
            ),
        }
    )


class _Status:
    task_status = 1
    stu_status = 2
    submit_time = 123
    is_finished = True


class _WordPkSession:
    username = "tester"
    user_type = 102

    def __init__(self) -> None:
        self.submitted: dict | None = None

    def bucket_words(self, bucket: str) -> list[dict]:
        assert bucket == "BUCKET-1"
        return [_word(index) for index in range(1, 11)]

    def submit_word_pk(self, job, score, correct, wrong, *, role_name="Red"):
        self.submitted = {
            "job": job,
            "score": score,
            "correct": correct,
            "wrong": wrong,
            "role_name": role_name,
        }
        return {"ok": True}

    def job_status(self, *args):
        return _Status()


def _feed(monkeypatch, answers: list[str]) -> None:
    iterator = iter(answers)

    def fake_input(prompt: str = "") -> str:
        return next(iterator)

    monkeypatch.setattr("builtins.input", fake_input)


def test_word_pk_flow_submits_score_and_split(monkeypatch) -> None:
    session = _WordPkSession()
    app = App(session, UI(color=False, clear_screen=False))
    app.course = {"courseId": 1, "courseName": "Test"}
    app.job = _job(word_pk=True)
    # score, total, role (default), confirm, pause
    _feed(monkeypatch, ["7", "10", "", "y", ""])

    app.word_pk_flow()

    assert session.submitted is not None
    assert session.submitted["score"] == 7
    assert len(session.submitted["correct"]) == 7
    assert len(session.submitted["wrong"]) == 3
    assert session.submitted["role_name"] == "Red"


class _ReadAloudSession:
    username = "tester"
    user_type = 102

    def __init__(self, paper_xml: str) -> None:
        self.paper_xml = paper_xml
        self.submitted = False

    def job_info(self, *args) -> dict:
        return {"paperId": "PAPER-1", "taskStatus": 0}

    def paper_detail(self, paper_id: str) -> str:
        assert paper_id == "PAPER-1"
        return self.paper_xml

    def submit_word_pk(self, *args, **kwargs):  # pragma: no cover - must not run
        self.submitted = True
        raise AssertionError("read-aloud flow must not submit")


def _readaloud_paper_xml() -> str:
    encoded = urllib.parse.quote_plus(ATTACHMENT)
    return (
        "<elements>"
        '<element id="Q1" type="3">'
        "<question_no>1</question_no>"
        "<question_text><![CDATA[Hello world.]]></question_text>"
        "<question_score>10.0</question_score>"
        "<question_type>9</question_type>"
        "<duration>82</duration>"
        f"<attachment><![CDATA[{encoded}]]></attachment>"
        "</element>"
        "</elements>"
    )


def test_readaloud_flow_saves_plan_without_submitting(monkeypatch, tmp_path: Path) -> None:
    session = _ReadAloudSession(_readaloud_paper_xml())
    app = App(session, UI(color=False, clear_screen=False))
    app.course = {"courseId": 1, "courseName": "Test"}
    app.job = _job(word_pk=False)

    mp3 = tmp_path / "q1.mp3"
    mp3.write_bytes(b"ID3\x04\x00\x00\x00\x00\x00\x00")

    saved: dict = {}
    plan_path = tmp_path / "plan.json"

    def fake_save_plan(plan, *, root=None):
        saved["plan"] = plan
        plan_path.write_text(
            json.dumps(plan.to_dict(), ensure_ascii=False), encoding="utf-8"
        )
        return plan_path

    monkeypatch.setattr(app_module, "save_plan", fake_save_plan)
    # mp3 path, pause
    _feed(monkeypatch, [str(mp3), ""])

    app.readaloud_flow()

    assert session.submitted is False
    assert "plan" in saved
    plan = saved["plan"]
    assert len(plan.questions) == 1
    assert len(plan.mp3_files) == 1
    assert plan.submission == "not_implemented"
    assert plan_path.exists()


class _JobListSession:
    username = "tester"
    user_type = 102

    def __init__(self, status_data: dict) -> None:
        self.status_data = status_data

    def jobs(self, course_id):
        return [_job(word_pk=False)]

    def course_finished_jobs(self, course_id):
        return self.status_data


def test_job_list_merges_finished_status() -> None:
    session = _JobListSession(
        {"finishedList": [{"studyTaskId": "JOB-1", "score": 8}]}
    )
    app = App(session, UI(color=False, clear_screen=False))
    app.course = {"courseId": 1, "courseName": "Test"}

    jobs = app._jobs_for_current_course()
    state = app._job_state(jobs[0])

    assert state.state == STATE_FINISHED
    assert state.score == 8
    assert "已交" in " ".join(app._job_tags(jobs[0]))


def test_job_list_marks_returned_job() -> None:
    session = _JobListSession({"returnedList": [{"studyTaskId": "JOB-1"}]})
    app = App(session, UI(color=False, clear_screen=False))
    app.course = {"courseId": 1, "courseName": "Test"}

    jobs = app._jobs_for_current_course()
    state = app._job_state(jobs[0])

    assert state.state == STATE_RETURNED
    assert "已退回" in " ".join(app._job_tags(jobs[0]))


class _DetailSession(_JobListSession):
    def job_info(self, *args):
        return {"taskStatus": 0, "stuStatus": 0, "paperId": "PAPER-1"}


def test_job_info_detail_overrides_finished_list() -> None:
    session = _DetailSession(
        {"finishedList": [{"studyTaskId": "JOB-1", "score": 8}]}
    )
    app = App(session, UI(color=False, clear_screen=False))
    app.course = {"courseId": 1, "courseName": "Test"}

    jobs = app._jobs_for_current_course()
    app._fetch_job_detail(jobs[0])
    state = app._job_state(jobs[0])

    assert state.state == STATE_UNFINISHED
    assert state.source == "job_info"
