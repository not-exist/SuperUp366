from __future__ import annotations

import json

from up366 import Job
from up366.job import JobStatus


def test_job_exposes_core_fields_and_parses_ext_param():
    raw = {
        "jobId": "JOB-1",
        "jobName": "Unit 4 单词PK",
        "courseId": 9222783,
        "contentId": "CONTENT-1",
        "type": 1,
        "beginTime": "2026-11-01T00:00:00Z",
        "endTime": "2026-11-10T00:00:00Z",
        "extParam": json.dumps(
            {"bookId": "BOOK-1", "chapterId": "CHAPTER-1", "taskScore": 100}
        ),
    }

    job = Job.from_dict(raw)

    assert job.job_id == "JOB-1"
    assert job.study_task_id == "JOB-1"
    assert job.name == "Unit 4 单词PK"
    assert job.course_id == 9222783
    assert job.content_id == "CONTENT-1"
    assert job.book_id == "BOOK-1"
    assert job.chapter_id == "CHAPTER-1"
    assert job.task_score == 100
    assert job.type == 1
    assert job.to_dict() == raw


def test_job_accepts_ext_param_as_dict():
    job = Job.from_dict(
        {
            "jobId": "JOB-1",
            "courseId": 1,
            "extParam": {"bookId": "BOOK-1"},
        }
    )

    assert job.book_id == "BOOK-1"
    assert job.ext_param == {"bookId": "BOOK-1"}


def test_job_detects_word_pk_from_xot_param():
    word_pk = Job.from_dict(
        {
            "jobId": "JOB-PK",
            "courseId": 1,
            "extParam": json.dumps(
                {
                    "xotParam": json.dumps(
                        {"pkBucketId": "BUCKET-1", "pkType": "limit"}
                    )
                }
            ),
        }
    )
    ordinary = Job.from_dict(
        {
            "jobId": "JOB-OTHER",
            "courseId": 1,
            "extParam": json.dumps(
                {"xotParam": json.dumps({"reportType": 0})}
            ),
        }
    )

    assert word_pk.is_word_pk is True
    assert word_pk.xot_param["pkBucketId"] == "BUCKET-1"
    assert ordinary.is_word_pk is False


def test_job_finished_accepts_numeric_strings():
    finished_by_task = Job.from_dict(
        {
            "jobId": "JOB-1",
            "extParam": json.dumps({"taskStatus": "1"}),
        }
    )
    finished_by_stu = Job.from_dict(
        {
            "jobId": "JOB-2",
            "extParam": json.dumps({"stuStatus": "3"}),
        }
    )
    unfinished = Job.from_dict(
        {
            "jobId": "JOB-3",
            "extParam": json.dumps({"taskStatus": "0", "stuStatus": "1"}),
        }
    )

    assert finished_by_task.is_finished is True
    assert finished_by_stu.is_finished is True
    assert unfinished.is_finished is False


def test_job_status_finished_accepts_numeric_strings():
    status = JobStatus.from_dict(
        {"taskStatus": "1", "stuStatus": "0", "submitTime": 1}
    )

    assert status.is_finished is True
