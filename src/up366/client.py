from __future__ import annotations

import hashlib
import json
import os
import time
import urllib.parse
import uuid
from collections.abc import Mapping, Sequence
from typing import Any

import httpx

from .book import BookAPI
from .endpoints import ENDPOINTS
from .errors import (
    Up366APIError,
    Up366AuthError,
    Up366Error,
    Up366HTTPError,
    Up366RequestError,
    Up366ResponseError,
)
from .form import form_encode
from .job import Job
from .job_api import JobAPIMixin
from .words import build_word_infos
from .wrongnote import WrongNoteAPI

__all__ = [
    "Job",
    "Up366",
    "Up366APIError",
    "Up366AuthError",
    "Up366Error",
    "Up366HTTPError",
    "Up366RequestError",
    "Up366ResponseError",
    "login",
    "up366",
]

SSO_BASE = "https://sso.up366.cn"
LOGIN_PATH = "/v2/tickets-v3"
SERVICE_TICKET_PATH = "/v1/serviceTicket"
# Exact value sent by the official PC student client.
USER_AGENT = "PC-Up366-Student 6.11.0"
PASSWORD_SALT = "fa&s*l%$k!fq$k!ld@fjlk"
LOGIN_UT_KEYWORD = "loginEncryptionParameter"
DEFAULT_TIMEOUT = 20.0


def _md5(value: str) -> str:
    return hashlib.md5(value.encode("utf-8")).hexdigest()


def _password_hash(password: str) -> str:
    return _md5(_md5(password) + PASSWORD_SALT)


def _login_ut(username: str, password_hash: str) -> str:
    ts = _md5(str(int(time.time() * 1000)))
    return (
        ts[:10]
        + _md5(username + password_hash + LOGIN_UT_KEYWORD + ts)
        + ts[10:]
    )


def _default_client_id() -> str:
    configured = os.environ.get("UP366_CLIENT_ID")
    if configured:
        return configured
    return hashlib.sha1(str(uuid.getnode()).encode("ascii")).hexdigest().upper()


def _headers(client_id: str, tgt: str) -> dict[str, str]:
    return {
        "user-agent": USER_AGENT,
        "x-requested-with": "PC",
        "X-App-Name": "student-pc",
        "clientId": client_id,
        "u3r": uuid.uuid4().hex,
        "u3t": tgt,
        "Authorization": "",
    }


class Up366(BookAPI, WrongNoteAPI, JobAPIMixin):
    """Authenticated up366 session.

    Stores the TGT plus the stable values needed to build subsequent request
    headers. The plaintext password is never kept; only its client-compatible
    hash is stored for re-login.
    """

    def __init__(
        self,
        *,
        username: str,
        password_hash: str,
        tgt: str,
        client_id: str,
        user_type: int | None = None,
        cookies: httpx.Cookies | dict | None = None,
    ):
        self.username = username
        self.password_hash = password_hash
        self.tgt = tgt
        self.client_id = client_id
        self.user_type = user_type
        self.cookies = (
            cookies
            if isinstance(cookies, httpx.Cookies)
            else httpx.Cookies(cookies or {})
        )

    @property
    def headers(self) -> dict[str, str]:
        """Return the fixed headers for a new authenticated request."""
        return _headers(self.client_id, self.tgt)

    def _parse_response(self, response: httpx.Response) -> dict:
        try:
            payload = response.json()
        except ValueError as exc:
            raise Up366ResponseError(
                f"HTTP {response.status_code}: invalid JSON response"
            ) from exc
        if not isinstance(payload, dict):
            raise Up366ResponseError("response is not a JSON object")
        return payload

    def _service_ticket(self, service: str) -> str:
        try:
            response = httpx.post(
                SSO_BASE + SERVICE_TICKET_PATH,
                data={"service": service},
                headers=self.headers,
                cookies=self.cookies,
                timeout=DEFAULT_TIMEOUT,
            )
        except httpx.HTTPError as exc:
            raise Up366RequestError(
                f"service ticket request failed: {exc}"
            ) from exc

        self.cookies.update(response.cookies)
        payload = self._parse_response(response)
        result = payload.get("result")
        result = result if isinstance(result, dict) else {}
        if result.get("code") != -24:
            raise Up366APIError(
                result.get("code"),
                result.get("msg") or "failed to get service ticket",
            )

        data = payload.get("data")
        data = data if isinstance(data, dict) else {}
        ticket = data.get("serverTicket")
        if not ticket:
            raise Up366ResponseError(
                "service ticket response did not contain data.serverTicket"
            )
        return ticket

    def _request(
        self,
        method: str,
        url: str,
        *,
        data: dict | None = None,
        params: dict | None = None,
        retry_auth: bool = True,
    ) -> dict:
        request_url = url
        if params:
            query = urllib.parse.urlencode(params, doseq=True)
            if query:
                separator = "&" if "?" in url else "?"
                request_url = f"{url}{separator}{query}"

        body = (
            form_encode(data)
            if method.upper() == "POST" and data is not None
            else data
        )
        try:
            response = httpx.request(
                method,
                request_url,
                data=body,
                headers=self.headers,
                cookies=self.cookies,
                timeout=DEFAULT_TIMEOUT,
            )
        except httpx.HTTPError as exc:
            raise Up366RequestError(
                f"{method} {request_url} failed: {exc}"
            ) from exc

        self.cookies.update(response.cookies)
        if response.status_code != 200:
            message = response.text[:200] or response.reason_phrase
            raise Up366HTTPError(response.status_code, request_url, message)

        payload = self._parse_response(response)
        result = payload.get("result")
        result = result if isinstance(result, dict) else {}
        code = result.get("code")
        message = result.get("msg") or "unknown error"

        if code in (-21, -22) and retry_auth:
            ticket = self._service_ticket(request_url)
            separator = "&" if "?" in request_url else "?"
            return self._request(
                method,
                f"{request_url}{separator}ticket={urllib.parse.quote(ticket)}",
                data=data,
                retry_auth=False,
            )
        if code != 0:
            raise Up366APIError(code, message)
        return payload

    @staticmethod
    def _list_data(payload: dict, key: str) -> list[dict]:
        data = payload.get("data")
        if not isinstance(data, dict):
            raise Up366ResponseError(f"response did not contain a data object for {key}")
        items = data.get(key)
        if items is None:
            return []
        if not isinstance(items, list):
            raise Up366ResponseError(f"response data.{key} is not a list")
        return items

    @staticmethod
    def _array_data(payload: dict) -> list:
        data = payload.get("data")
        if not isinstance(data, list):
            raise Up366ResponseError("response data is not a list")
        return data

    @staticmethod
    def _dict_data(payload: dict) -> dict:
        data = payload.get("data")
        if not isinstance(data, dict):
            raise Up366ResponseError("response data is not an object")
        return data

    @staticmethod
    def _page_data(payload: dict) -> tuple[list, dict]:
        data = payload.get("data")
        if not isinstance(data, dict):
            raise Up366ResponseError("response data is not a page object")
        items = data.get("list")
        pager = data.get("pager")
        if not isinstance(items, list) or not isinstance(pager, dict):
            raise Up366ResponseError("response data.list/pager is malformed")
        return items, pager

    def courses(self) -> list[dict]:
        """Return the current student's course list."""
        endpoint = ENDPOINTS["courses"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"status": 1, "pager.pageSize": 999},
        )
        return self._list_data(payload, "list")

    def jobs(self, course_id: int | str) -> list[Job]:
        """Return the course's homework/task list (data.allTaskList)."""
        endpoint = ENDPOINTS["jobs"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"courseId": course_id, "pageIdFlag": 1, "version": ""},
        )
        return [
            Job.from_dict(item)
            for item in self._list_data(payload, "allTaskList")
        ]

    def job_info(
        self,
        course_id: int | str,
        job_id: str,
        content_id: str,
        book_id: str,
    ) -> dict:
        """Return one job's detail, including taskStatus and paperId."""
        endpoint = ENDPOINTS["job_info"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "courseId": course_id,
                "jobId": job_id,
                "studyTaskId": job_id,
                "contentId": content_id,
                "bookId": book_id,
            },
        )
        data = payload.get("data")
        if not isinstance(data, dict):
            raise Up366ResponseError(
                "job/info response did not contain a data object"
            )
        return data

    def bucket_words(self, bucket_id: str) -> list[dict]:
        """Return all words in a word-PK bucket, flattened in server order."""
        endpoint = ENDPOINTS["bucket_words"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"source": 1, "bucketId": bucket_id},
        )
        data = payload.get("data")
        if not isinstance(data, dict):
            raise Up366ResponseError(
                "bucket detail response did not contain a data object"
            )

        words: list[dict] = []
        for group in data.get("contentList") or []:
            if not isinstance(group, dict):
                continue
            for entry in group.get("entryList") or []:
                if isinstance(entry, dict):
                    words.append(entry)
        return words

    @staticmethod
    def _as_word_mapping(word: Mapping[str, Any] | str) -> dict[str, Any]:
        if isinstance(word, Mapping):
            return dict(word)
        if isinstance(word, str):
            return {"entry": word, "answer": word}
        raise TypeError(
            "word items must be mappings or strings, "
            f"got {type(word).__name__}"
        )

    @classmethod
    def _compact_word_infos(
        cls,
        correct_words: Sequence[Mapping[str, Any] | str],
        wrong_words: Sequence[Mapping[str, Any] | str],
    ) -> list[dict[str, Any]]:
        infos: list[dict[str, Any]] = []
        for answer_status, words in ((1, correct_words), (0, wrong_words)):
            for word in words:
                if isinstance(word, Mapping):
                    entry_id = word.get("entryId")
                    if entry_id is None:
                        entry_id = word.get("pkid")
                    info = (
                        {"entryId": entry_id}
                        if entry_id is not None
                        else dict(word)
                    )
                elif isinstance(word, str):
                    info = {"entryId": word}
                else:
                    raise TypeError(
                        "word items must be mappings or strings, "
                        f"got {type(word).__name__}"
                    )
                info["answerStatus"] = answer_status
                infos.append(info)
        return infos

    def submit_word_pk(
        self,
        job: Job,
        score: int,
        correct_words: Sequence[Mapping[str, Any] | str],
        wrong_words: Sequence[Mapping[str, Any] | str],
        *,
        role_name: str = "Red",
        duration: str = "",
        compact: bool = False,
        allow_get_fallback: bool = True,
    ) -> dict:
        """Submit one word-PK task result for ``job``.

        POST is preferred. The GET fallback is only used when the server
        returns HTTP 404/405, and it can only carry compact word records.
        """
        if not isinstance(job, Job):
            raise TypeError("job must be a Job instance")
        if not job.is_word_pk:
            raise ValueError("job is not a word PK job")
        if not job.course_id or not job.job_id:
            raise ValueError("job is missing courseId or jobId")

        def make_params(word_infos: list[dict]) -> dict:
            submit_json = {
                "totalNum": len(word_infos),
                "rightNum": len(correct_words),
                "score": score,
                "wordInfos": word_infos,
                "duration": duration,
            }
            return {
                "courseId": job.course_id,
                "studyTaskId": job.job_id,
                "roleName": role_name,
                "roleType": "0",
                "submitJson": json.dumps(
                    submit_json, separators=(",", ":"), ensure_ascii=False
                ),
                "wrongLength": len(wrong_words),
                "score": score,
            }

        if compact:
            word_infos = self._compact_word_infos(correct_words, wrong_words)
        else:
            word_infos = build_word_infos(
                [self._as_word_mapping(w) for w in correct_words],
                [self._as_word_mapping(w) for w in wrong_words],
            )

        endpoint = ENDPOINTS["word_pk_submit"]
        url = endpoint.base_url + endpoint.path
        params = make_params(word_infos)

        try:
            payload = self._request(endpoint.method, url, data=params)
        except Up366HTTPError as exc:
            if not allow_get_fallback or exc.status_code not in (404, 405):
                raise
            params = make_params(
                self._compact_word_infos(correct_words, wrong_words)
            )
            payload = self._request("GET", url, params=params)

        data = payload.get("data")
        return data if isinstance(data, (dict, list)) else {}


def login(username: str, password: str) -> Up366:
    """Log in with username/password and return an authenticated session.

    Set ``UP366_CLIENT_ID`` to use a specific device id; otherwise a stable
    machine-derived id is generated.
    """
    client_id = _default_client_id()
    password_hash = _password_hash(password)
    try:
        response = httpx.post(
            SSO_BASE + LOGIN_PATH,
            data={
                "username": username,
                "password": password_hash,
                "ut": _login_ut(username, password_hash),
            },
            headers=_headers(client_id, ""),
            timeout=DEFAULT_TIMEOUT,
        )
    except httpx.HTTPError as exc:
        raise Up366Error(f"login request failed: {exc}") from exc

    try:
        payload = response.json()
    except ValueError as exc:
        raise Up366AuthError(
            None, f"HTTP {response.status_code}: invalid JSON response"
        ) from exc

    result = payload.get("result") if isinstance(payload, dict) else None
    data = payload.get("data") if isinstance(payload, dict) else None
    result = result if isinstance(result, dict) else {}
    data = data if isinstance(data, dict) else {}
    code = result.get("code")
    message = result.get("msg") or f"HTTP {response.status_code}"

    if response.status_code != 200:
        raise Up366AuthError(code, message)
    if code != -26:
        raise Up366AuthError(code, message)

    tgt = data.get("tgt")
    if not tgt:
        raise Up366AuthError(code, "login response did not contain data.tgt")

    user_type = data.get("uType")
    return Up366(
        username=username,
        password_hash=password_hash,
        tgt=tgt,
        client_id=client_id,
        user_type=int(user_type) if user_type is not None else None,
        cookies=httpx.Cookies(response.cookies),
    )


up366 = Up366
