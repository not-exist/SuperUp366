from __future__ import annotations

import httpx

import up366.client as client_module
from up366 import Up366


def _response(payload: dict) -> httpx.Response:
    return httpx.Response(
        200,
        json=payload,
        request=httpx.Request(
            "POST",
            "https://words-v2-api.up366.cn/client/sync/teaching/bucket/detail-info",
        ),
    )


def _session() -> Up366:
    return Up366(
        username="alice",
        password_hash="hash",
        tgt="TGT-123",
        client_id="CID-TEST",
    )


def test_bucket_words_posts_and_flattens(monkeypatch):
    captured: dict[str, object] = {}

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data)
        return _response(
            {
                "result": {"code": 0, "msg": "ok"},
                "data": {
                    "contentList": [
                        {"entryList": [{"entryId": 1}, {"entryId": 2}]},
                        {"entryList": [{"entryId": 3}]},
                    ]
                },
            }
        )

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    words = _session().bucket_words("BUCKET-1")

    assert [word["entryId"] for word in words] == [1, 2, 3]
    assert captured["method"] == "POST"
    assert captured["url"] == (
        "https://words-v2-api.up366.cn/client/sync/teaching/bucket/detail-info"
    )
    assert captured["data"] == {"source": "1", "bucketId": "BUCKET-1"}
