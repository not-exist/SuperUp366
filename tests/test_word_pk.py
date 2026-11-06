from __future__ import annotations

import json
import urllib.parse

import httpx
import pytest

import up366.client as client_module
from up366 import Job, Up366, Up366APIError


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
    )


def _job() -> Job:
    return Job.from_dict(
        {
            "jobId": "JOB-1",
            "jobName": "Unit 4 单词PK",
            "courseId": 9222783,
            "contentId": "CONTENT-1",
            "extParam": json.dumps(
                {
                    "bookId": "BOOK-1",
                    "xotParam": json.dumps(
                        {"pkBucketId": "BUCKET-1", "pkType": "limit"}
                    ),
                }
            ),
        }
    )


def _word(entry_id: int, entry: str, paraphrase: str) -> dict:
    return {
        "entryId": entry_id,
        "entry": entry,
        "paraphrase": paraphrase,
        "usPhonetic": f"[{entry}]",
        "usVoiceFile": f"entryfilesv2/{entry_id}/us.mp3",
        "sourceType": 1,
    }


def test_submit_word_pk_posts_full_details(monkeypatch):
    captured: dict[str, object] = {}

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(
            method=method,
            url=url,
            data=data,
            headers=headers,
            timeout=timeout,
            cookies=cookies,
        )
        return _response(
            {"result": {"code": 0, "msg": "ok"}, "data": {"submitted": True}},
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().submit_word_pk(
        _job(),
        8,
        [
            _word(1, "apple", "n. 苹果"),
            _word(2, "banana", "n. 香蕉"),
        ],
        [_word(3, "pear", "n. 梨")],
    )

    assert result == {"submitted": True}
    assert captured["method"] == "POST"
    assert captured["url"] == "https://wordsbtl-api.up366.cn/client/wordsbtl/student/submit"

    body = captured["data"]
    assert body["courseId"] == "9222783"
    assert body["studyTaskId"] == "JOB-1"
    assert body["roleName"] == "Red"
    assert body["roleType"] == "0"
    assert body["wrongLength"] == "1"
    assert body["score"] == "8"

    submit_json = json.loads(body["submitJson"])
    assert submit_json["totalNum"] == 3
    assert submit_json["rightNum"] == 2
    assert submit_json["score"] == 8
    assert submit_json["duration"] == ""

    infos = submit_json["wordInfos"]
    assert [item["answerStatus"] for item in infos] == [1, 1, 0]
    assert infos[0]["entryId"] == 1
    assert infos[0]["en"] == "apple"
    assert infos[0]["cn"] == "n. 苹果"
    assert infos[0]["phonetic"] == "[apple]"
    assert infos[0]["audio"] == "https://word-cdn-alyun.up366.cn/entryfilesv2/1/us.mp3"
    assert infos[0]["localId"] == 0
    assert infos[0]["answer"] == "apple"
    assert "answerTime" in infos[0]
    assert infos[2]["entryId"] == 3
    assert infos[2]["answerStatus"] == 0


def test_submit_word_pk_falls_back_to_get_on_405(monkeypatch):
    calls: list[tuple[str, str]] = []

    def fake_request(method, url, *, data, headers, timeout, cookies):
        calls.append((method, url))
        if len(calls) == 1:
            return httpx.Response(
                405,
                text="method not allowed",
                request=httpx.Request(method, url),
            )
        return _response(
            {"result": {"code": 0, "msg": "ok"}, "data": {}},
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    _session().submit_word_pk(
        _job(),
        1,
        [{"entryId": "E1", "entry": "apple"}],
        [{"entryId": "E2", "entry": "pear"}],
    )

    assert [method for method, _ in calls] == ["POST", "GET"]
    assert calls[0][1] == "https://wordsbtl-api.up366.cn/client/wordsbtl/student/submit"

    query = urllib.parse.parse_qs(urllib.parse.urlparse(calls[1][1]).query)
    submit_json = json.loads(query["submitJson"][0])
    assert submit_json["wordInfos"] == [
        {"entryId": "E1", "answerStatus": 1},
        {"entryId": "E2", "answerStatus": 0},
    ]


def test_submit_word_pk_does_not_fall_back_on_business_error(monkeypatch):
    calls: list[str] = []

    def fake_request(method, url, *, data, headers, timeout, cookies):
        calls.append(method)
        return _response(
            {"result": {"code": -13, "msg": "重复作答"}},
            method=method,
            url=url,
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    with pytest.raises(Up366APIError) as excinfo:
        _session().submit_word_pk(
            _job(),
            1,
            [{"entryId": "E1", "entry": "apple"}],
            [],
        )

    assert excinfo.value.code == -13
    assert calls == ["POST"]


def test_submit_word_pk_accepts_plain_word_strings(monkeypatch):
    captured: dict[str, object] = {}

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, data=data)
        return _response({"result": {"code": 0, "msg": "ok"}, "data": {}})

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    _session().submit_word_pk(_job(), 1, ["apple"], ["pear"])

    submit_json = json.loads(captured["data"]["submitJson"])
    infos = submit_json["wordInfos"]
    assert infos[0]["en"] == "apple"
    assert infos[0]["answerStatus"] == 1
    assert infos[1]["en"] == "pear"
    assert infos[1]["answerStatus"] == 0


def test_submit_word_pk_rejects_non_word_pk_job(monkeypatch):
    called = False

    def fake_request(method, url, *, data, headers, timeout, cookies):
        nonlocal called
        called = True
        return _response({"result": {"code": 0, "msg": "ok"}, "data": {}})

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    ordinary_job = Job.from_dict(
        {
            "jobId": "JOB-OTHER",
            "jobName": "普通跟读",
            "courseId": 9222783,
            "contentId": "CONTENT-OTHER",
            "extParam": json.dumps(
                {"xotParam": json.dumps({"reportType": 0})}
            ),
        }
    )

    with pytest.raises(ValueError, match="word PK"):
        _session().submit_word_pk(ordinary_job, 1, ["apple"], [])

    assert called is False
