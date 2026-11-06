from __future__ import annotations

import dataclasses

import pytest

from up366.endpoints import ENDPOINTS, Endpoint


def test_endpoint_defaults_declare_post_form_read_only():
    endpoint = Endpoint(name="probe", base_url="https://example.test", path="/x")

    assert endpoint.method == "POST"
    assert endpoint.body == "form"
    assert endpoint.fallback is None
    assert endpoint.write is False


def test_endpoint_is_frozen():
    endpoint = Endpoint(name="probe", base_url="https://example.test", path="/x")

    with pytest.raises(dataclasses.FrozenInstanceError):
        endpoint.method = "GET"  # type: ignore[misc]


def test_endpoints_registry_keys_match_endpoint_names():
    for key, endpoint in ENDPOINTS.items():
        assert key == endpoint.name


@pytest.mark.parametrize(
    ("name", "base_url", "path", "fallback", "write"),
    [
        (
            "courses",
            "https://course-api.up366.cn",
            "/client/course/list-of-student/v2",
            None,
            False,
        ),
        (
            "jobs",
            "https://studytask-api.up366.cn",
            "/client/student/course/job",
            None,
            False,
        ),
        (
            "course_finished_jobs",
            "https://studytask-api.up366.cn",
            "/client/student/all/course/finish/job/v2",
            None,
            False,
        ),
        (
            "job_info",
            "https://studytask-api.up366.cn",
            "/client/student/job/info",
            None,
            False,
        ),
        (
            "bucket_words",
            "https://words-v2-api.up366.cn",
            "/client/sync/teaching/bucket/detail-info",
            None,
            False,
        ),
        (
            "word_pk_submit",
            "https://wordsbtl-api.up366.cn",
            "/client/wordsbtl/student/submit",
            "GET",
            True,
        ),
    ],
)
def test_endpoints_registry_declares_known_apis(
    name, base_url, path, fallback, write
):
    endpoint = ENDPOINTS[name]

    assert endpoint.base_url == base_url
    assert endpoint.path == path
    assert endpoint.method == "POST"
    assert endpoint.body == "form"
    assert endpoint.fallback == fallback
    assert endpoint.write is write
