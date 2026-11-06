from __future__ import annotations

import json
from datetime import datetime, timezone

from up366 import Job
from up366_tui.jobstatus import (
    STATE_EXPIRED,
    STATE_FINISHED,
    STATE_RETURNED,
    STATE_UNFINISHED,
    STATE_UNKNOWN,
    build_status_indexes,
    detail_state,
    parse_deadline,
    resolve_job_state,
)


def _job(**overrides) -> Job:
    raw = {
        "jobId": "JOB-1",
        "jobName": "Test job",
        "courseId": 1,
        "type": 1,
        "endTime": "2099-01-01T00:00:00Z",
        "extParam": json.dumps({}),
    }
    raw.update(overrides)
    return Job.from_dict(raw)


def test_build_status_indexes_maps_by_study_task_id() -> None:
    finished, returned = build_status_indexes(
        {
            "finishedList": [
                {"studyTaskId": "JOB-1", "score": 8},
                {"studyTaskId": ""},
                "not-a-mapping",
            ],
            "returnedList": [{"studyTaskId": "JOB-2"}],
        }
    )

    assert set(finished) == {"JOB-1"}
    assert finished["JOB-1"]["score"] == 8
    assert set(returned) == {"JOB-2"}


def test_empty_indexes_are_empty() -> None:
    assert build_status_indexes(None) == ({}, {})
    assert build_status_indexes({}) == ({}, {})


def test_finished_list_marks_job_finished() -> None:
    state = resolve_job_state(
        _job(), finished={"JOB-1": {"score": 8, "studyTaskId": "JOB-1"}}
    )

    assert state.state == STATE_FINISHED
    assert state.source == "finished_list"
    assert state.score == 8
    assert state.is_finished is True
    assert state.label == "已交"


def test_returned_list_wins_over_finished_and_detail() -> None:
    state = resolve_job_state(
        _job(),
        finished={"JOB-1": {}},
        returned={"JOB-1": {}},
        detail_status=STATE_FINISHED,
    )

    assert state.state == STATE_RETURNED
    assert state.source == "returned_list"


def test_job_info_detail_is_authoritative() -> None:
    state = resolve_job_state(
        _job(), finished={"JOB-1": {}}, detail_status=STATE_UNFINISHED
    )
    assert state.state == STATE_UNFINISHED
    assert state.source == "job_info"

    state = resolve_job_state(_job(), detail_status=STATE_FINISHED)
    assert state.state == STATE_FINISHED


def test_type_one_uses_task_status_only() -> None:
    finished = _job(
        type=1, extParam=json.dumps({"taskStatus": 1, "stuStatus": 3})
    )
    assert resolve_job_state(finished).state == STATE_FINISHED

    missing_task_status = _job(type=1, extParam=json.dumps({"stuStatus": 3}))
    assert resolve_job_state(missing_task_status).state == STATE_UNFINISHED


def test_type_three_uses_stu_status_only() -> None:
    finished = _job(type=3, extParam=json.dumps({"stuStatus": 3}))
    assert resolve_job_state(finished).state == STATE_FINISHED

    missing_stu_status = _job(type=3, extParam=json.dumps({"taskStatus": 1}))
    assert resolve_job_state(missing_stu_status).state == STATE_UNFINISHED


def test_status_flags_accept_numeric_strings() -> None:
    type_one = _job(type=1, extParam=json.dumps({"taskStatus": "1"}))
    type_three = _job(type=3, extParam=json.dumps({"stuStatus": "3"}))

    assert resolve_job_state(type_one).state == STATE_FINISHED
    assert resolve_job_state(type_three).state == STATE_FINISHED


def test_deadline_marks_unfinished_job_expired() -> None:
    job = _job(type=1, endTime="2020-01-01T00:00:00Z")
    state = resolve_job_state(
        job, now=datetime(2026, 1, 1, tzinfo=timezone.utc)
    )
    assert state.state == STATE_EXPIRED


def test_detail_unfinished_plus_expired_is_expired() -> None:
    job = _job(type=1, endTime="2020-01-01T00:00:00Z")
    state = resolve_job_state(
        job,
        detail_status=STATE_UNFINISHED,
        now=datetime(2026, 1, 1, tzinfo=timezone.utc),
    )
    assert state.state == STATE_EXPIRED


def test_flag_unfinished_plus_expired_is_expired() -> None:
    job = _job(
        type=1,
        endTime="2020-01-01T00:00:00Z",
        extParam=json.dumps({"taskStatus": 0}),
    )
    state = resolve_job_state(
        job, now=datetime(2026, 1, 1, tzinfo=timezone.utc)
    )
    assert state.state == STATE_EXPIRED


def test_future_deadline_keeps_type_one_unfinished() -> None:
    state = resolve_job_state(
        _job(type=1),
        now=datetime(2026, 1, 1, tzinfo=timezone.utc),
    )
    assert state.state == STATE_UNFINISHED


def test_unknown_type_is_unknown() -> None:
    assert resolve_job_state(_job(type=2)).state == STATE_UNKNOWN


def test_detail_state_reads_job_info_fields() -> None:
    assert detail_state({"taskStatus": 1, "stuStatus": 0}) == STATE_FINISHED
    assert detail_state({"taskStatus": 0, "stuStatus": 2}) == STATE_FINISHED
    assert detail_state({"taskStatus": 0, "stuStatus": 1}) == STATE_UNFINISHED
    assert detail_state({}) is None


def test_parse_deadline_handles_z_naive_and_epoch() -> None:
    assert parse_deadline("2026-11-10T00:00:00Z") == datetime(
        2026, 11, 10, tzinfo=timezone.utc
    )
    assert parse_deadline("2026-11-10T00:00:00") == datetime(
        2026, 11, 10, tzinfo=timezone.utc
    )
    assert parse_deadline(1793714840149) == datetime.fromtimestamp(
        1793714840.149, tz=timezone.utc
    )
    assert parse_deadline("not a date") is None
