from __future__ import annotations

import re

import httpx
import pytest

import up366 as up366_package
import up366.client as client_module
from up366 import Up366, Up366AuthError, Up366Error, login

LOGIN_URL = "https://sso.up366.cn/v2/tickets-v3"
EXPECTED_PASSWORD_HASH = "e3d5506a677ff4ad438b2461b08646f2"
EXPECTED_UT = "8f5422e1ee5f3ede8fee55f9937f07267ac05f8b44eb6aae581bd979275206ce"


def _response(payload: dict, status_code: int = 200, headers=None) -> httpx.Response:
    return httpx.Response(
        status_code,
        json=payload,
        headers=headers,
        request=httpx.Request("POST", LOGIN_URL),
    )


def test_login_posts_expected_request_and_stores_auth(monkeypatch):
    captured: dict[str, object] = {}

    def fake_post(url, *, data, headers, timeout):
        captured["url"] = url
        captured["data"] = data
        captured["headers"] = headers
        captured["timeout"] = timeout
        return _response(
            {
                "result": {"code": -26, "msg": "ok"},
                "data": {"tgt": "TGT-123", "uType": 102},
            }
        )

    monkeypatch.setattr(client_module.httpx, "post", fake_post)
    monkeypatch.setattr(client_module.time, "time", lambda: 1700000000.123)
    monkeypatch.setenv("UP366_CLIENT_ID", "CID-TEST")

    session = login("alice", "secret")

    assert isinstance(session, Up366)
    assert up366_package.up366 is Up366
    assert captured["url"] == LOGIN_URL
    assert captured["data"] == {
        "username": "alice",
        "password": EXPECTED_PASSWORD_HASH,
        "ut": EXPECTED_UT,
    }
    assert captured["timeout"] == 20.0

    headers = captured["headers"]
    assert headers["user-agent"] == "PC-Up366-Student 6.11.0"
    assert headers["x-requested-with"] == "PC"
    assert headers["X-App-Name"] == "student-pc"
    assert headers["clientId"] == "CID-TEST"
    assert headers["u3t"] == ""
    assert headers["Authorization"] == ""
    assert re.fullmatch(r"[0-9a-f]{32}", headers["u3r"])

    assert session.tgt == "TGT-123"
    assert session.username == "alice"
    assert session.password_hash == EXPECTED_PASSWORD_HASH
    assert session.client_id == "CID-TEST"
    assert session.user_type == 102
    assert session.headers["u3t"] == "TGT-123"
    assert session.headers["user-agent"] == "PC-Up366-Student 6.11.0"
    assert re.fullmatch(r"[0-9a-f]{32}", session.headers["u3r"])
    assert not hasattr(session, "password")


def test_login_sends_exact_client_headers(monkeypatch):
    seen: dict[str, str] = {}

    def handler(request: httpx.Request) -> httpx.Response:
        seen.update(dict(request.headers))
        return httpx.Response(
            200,
            json={
                "result": {"code": -26, "msg": "ok"},
                "data": {"tgt": "TGT-123", "uType": 102},
            },
            request=request,
        )

    def fake_post(url, *, data, headers, timeout):
        with httpx.Client(transport=httpx.MockTransport(handler)) as client:
            return client.post(url, data=data, headers=headers, timeout=timeout)

    monkeypatch.setattr(client_module.httpx, "post", fake_post)
    monkeypatch.setenv("UP366_CLIENT_ID", "CID-ACTUAL")

    login("alice", "secret")

    assert seen["user-agent"] == "PC-Up366-Student 6.11.0"
    assert seen["x-requested-with"] == "PC"
    assert seen["x-app-name"] == "student-pc"
    assert seen["clientid"] == "CID-ACTUAL"
    assert seen["u3t"] == ""
    assert seen["authorization"] == ""
    assert re.fullmatch(r"[0-9a-f]{32}", seen["u3r"])


def test_login_captures_session_cookies(monkeypatch):
    def fake_post(url, *, data, headers, timeout):
        return _response(
            {
                "result": {"code": -26, "msg": "ok"},
                "data": {"tgt": "TGT-123", "uType": 102},
            },
            headers=[
                ("set-cookie", "CASTGC=abc; Path=/"),
                ("set-cookie", "SESSION=def; Path=/"),
            ],
        )

    monkeypatch.setattr(client_module.httpx, "post", fake_post)
    monkeypatch.setenv("UP366_CLIENT_ID", "CID-TEST")

    session = login("alice", "secret")

    assert session.cookies["CASTGC"] == "abc"
    assert session.cookies["SESSION"] == "def"


def test_login_raises_auth_error_for_rejected_credentials(monkeypatch):
    monkeypatch.setattr(
        client_module.httpx,
        "post",
        lambda *args, **kwargs: _response(
            {"result": {"code": -27, "msg": "用户名或密码错误"}}
        ),
    )

    with pytest.raises(Up366AuthError) as excinfo:
        login("alice", "wrong")

    assert excinfo.value.code == -27
    assert "用户名或密码错误" in str(excinfo.value)


def test_login_rejects_response_without_tgt(monkeypatch):
    monkeypatch.setattr(
        client_module.httpx,
        "post",
        lambda *args, **kwargs: _response(
            {"result": {"code": -26, "msg": "ok"}, "data": {"uType": 102}}
        ),
    )

    with pytest.raises(Up366AuthError) as excinfo:
        login("alice", "secret")

    assert excinfo.value.code == -26
    assert "tgt" in str(excinfo.value).lower()


def test_login_wraps_network_error(monkeypatch):
    def fake_post(url, **kwargs):
        raise httpx.ConnectError(
            "connection refused", request=httpx.Request("POST", url)
        )

    monkeypatch.setattr(client_module.httpx, "post", fake_post)

    with pytest.raises(Up366Error) as excinfo:
        login("alice", "secret")

    assert "connection refused" in str(excinfo.value)
