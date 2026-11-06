from __future__ import annotations

import json
import urllib.parse
from pathlib import Path

import pytest

from up366 import Job
from up366_tui.paper import parse_paper_xml
from up366_tui.readaloud import (
    build_manual_plan,
    build_plan,
    save_plan,
    submit_plan,
)

ATTACHMENT = (
    "<root><paragraph>"
    '<sentences id="1"><content><![CDATA[Hello world.]]></content></sentences>'
    "</paragraph></root>"
)


def _job() -> Job:
    return Job.from_dict(
        {
            "jobId": "JOB-1",
            "jobName": "Unit 4 逐句跟读",
            "courseId": 9222783,
            "contentId": "CONTENT-1",
            "extParam": json.dumps(
                {"bookId": "BOOK-1", "paperId": "PAPER-1"}
            ),
        }
    )


def _paper_xml(count: int = 2, question_type: int = 9) -> str:
    encoded = urllib.parse.quote_plus(ATTACHMENT)
    elements = []
    for index in range(1, count + 1):
        elements.append(
            f'  <element id="Q{index}" type="3">\n'
            f"    <question_no>{index}</question_no>\n"
            "    <question_text><![CDATA[Hello world.]]></question_text>\n"
            "    <question_score>10.0</question_score>\n"
            f"    <question_type>{question_type}</question_type>\n"
            "    <duration>82</duration>\n"
            f"    <attachment><![CDATA[{encoded}]]></attachment>\n"
            "  </element>"
        )
    return "<elements>\n" + "\n".join(elements) + "\n</elements>\n"


def _mp3(tmp_path: Path, name: str) -> Path:
    path = tmp_path / name
    path.write_bytes(b"ID3\x04\x00\x00\x00\x00\x00\x00")
    return path


def test_build_plan_records_questions_and_files(tmp_path: Path) -> None:
    job = _job()
    paper = parse_paper_xml(_paper_xml(2))
    paths = [_mp3(tmp_path, "q1.mp3"), _mp3(tmp_path, "q2.mp3")]

    plan = build_plan(
        job=job,
        paper=paper,
        paths=paths,
        course_id=123,
        paper_id="PAPER-1",
    )

    assert plan.job_id == "JOB-1"
    assert plan.job_name == "Unit 4 逐句跟读"
    assert plan.course_id == "123"
    assert plan.content_id == "CONTENT-1"
    assert plan.book_id == "BOOK-1"
    assert plan.paper_id == "PAPER-1"
    assert plan.source == "paper"
    assert plan.submission == "not_implemented"
    assert len(plan.questions) == 2
    assert plan.questions[0]["element_id"] == "Q1"
    assert plan.questions[0]["sentence_count"] == 1
    assert len(plan.mp3_files) == 2
    assert plan.mp3_files[0]["name"] == "q1.mp3"
    assert plan.mp3_files[0]["size"] > 0
    assert plan.mp3_files[0]["question_element_id"] == "Q1"


def test_save_plan_writes_json_and_never_overwrites(tmp_path: Path) -> None:
    plan = build_plan(
        job=_job(),
        paper=parse_paper_xml(_paper_xml(1)),
        paths=[_mp3(tmp_path, "q1.mp3")],
    )
    first = save_plan(plan, root=tmp_path / "plans")
    second = save_plan(plan, root=tmp_path / "plans")

    assert first != second
    data = json.loads(first.read_text(encoding="utf-8"))
    assert data["submission"] == "not_implemented"
    assert data["mp3_files"][0]["name"] == "q1.mp3"
    assert data["questions"][0]["element_id"] == "Q1"


def test_submit_plan_is_an_explicit_stub(tmp_path: Path) -> None:
    plan = build_manual_plan(
        _job(), [_mp3(tmp_path, "q1.mp3")], course_id=123
    )

    with pytest.raises(NotImplementedError, match="留空"):
        submit_plan(object(), plan)


def test_build_manual_plan_has_no_paper_metadata(tmp_path: Path) -> None:
    paths = [_mp3(tmp_path, "a.mp3"), _mp3(tmp_path, "b.mp3")]
    plan = build_manual_plan(
        _job(), paths, course_id=123, paper_id="PAPER-1"
    )

    assert plan.source == "manual"
    assert len(plan.questions) == 2
    assert plan.questions[0]["element_id"] == ""
    assert len(plan.mp3_files) == 2


def test_build_plan_rejects_wrong_file_count(tmp_path: Path) -> None:
    paper = parse_paper_xml(_paper_xml(2))
    path = _mp3(tmp_path, "only.mp3")

    with pytest.raises(ValueError, match="需要 2 段"):
        build_plan(job=_job(), paper=paper, paths=[path])


def test_build_plan_rejects_non_mp3(tmp_path: Path) -> None:
    paper = parse_paper_xml(_paper_xml(1))
    path = tmp_path / "q1.wav"
    path.write_bytes(b"RIFF")

    with pytest.raises(ValueError, match=".mp3"):
        build_plan(job=_job(), paper=paper, paths=[path])


def test_build_plan_requires_readaloud_questions(tmp_path: Path) -> None:
    paper = parse_paper_xml(_paper_xml(1, question_type=4))

    with pytest.raises(ValueError, match="没有跟读题"):
        build_plan(job=_job(), paper=paper, paths=[_mp3(tmp_path, "q1.mp3")])
