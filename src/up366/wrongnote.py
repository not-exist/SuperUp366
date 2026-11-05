"""Wrong-answer book (``wrongnote-api``) models and API mixin.

Host and transport
------------------
These endpoints live on ``https://wrongnote-api.up366.cn`` — a *different*
host from the ordinary book APIs (``book-api`` / ``studytask-api``). They are
not merged into those mixins on purpose. All ten endpoints are POST.

Auth
----
Live matrix (2026-11-05): a login Cookie alone returns code 0; a one-time ST
``?ticket=`` alone also returns 0; ``u3t`` alone returns -21; no credentials
returns -21. So this host accepts **either** the session cookies or the ST
ticket, and does not use ``u3t`` by itself. The H5 client's axios wrapper
sends ``-21/-22`` failures to ``data.serverTicketUrl`` and replays with
``?ticket=``; inside the PC client Electron intercepts the SSO POST and
answers it with the PC TGT. ``Up366._request`` already carries cookies, so
the normal path works even before the ST retry.

Reads vs writes
---------------
``wrong_kill_paper`` (``questions/kill``) is a READ: the kill page fetches the
paper to render. ``submit_wrong_question`` (``question/submit``) is the only
destructive call; it defaults to ``dry_run=True`` and requires BOTH
``dry_run=False`` AND ``confirm=True`` to actually send. It never auto-retries.

Payloads
--------
``wrong_killed_questions`` / ``wrong_question_show`` build the exact H5 field
sets (including the pager query) and return ``WrongQuestion`` items. An empty
``data={}`` means "no questions" and yields an empty page.
"""
from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from .endpoints import ENDPOINTS
from .errors import Up366ResponseError

__all__ = [
    "WrongSubject",
    "WrongBook",
    "WrongKnowledge",
    "WrongQuestion",
    "WrongNotePage",
    "WrongSubjectStat",
    "WrongCourse",
    "WrongRank",
    "WrongNoteAPI",
]


class _Raw:
    """Thin raw-dict wrapper: keep the untouched payload, expose typed accessors."""

    def __init__(self, raw: Mapping[str, Any]):
        if not isinstance(raw, Mapping):
            raise TypeError(f"{type(self).__name__} raw data must be a mapping")
        self.raw = dict(raw)

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "_Raw":
        return cls(raw)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)

    def __repr__(self) -> str:
        return f"{type(self).__name__}({self.raw!r})"


class WrongSubject(_Raw):
    """A subject entry from ``wrong_subjects``."""

    @property
    def id(self) -> Any:
        return self.raw.get("id")

    @property
    def subject_name(self) -> Any:
        return self.raw.get("subjectName")

    @property
    def subject_id(self) -> Any:
        return self.raw.get("subjectId")

    @property
    def stage_id(self) -> Any:
        return self.raw.get("stageId")

    @property
    def wrong_count(self) -> Any:
        return self.raw.get("wrongCount")

    @property
    def killed_count(self) -> Any:
        return self.raw.get("killedCount")

    @property
    def killed_count_lately(self) -> Any:
        return self.raw.get("killedCountLately")


class WrongBook(_Raw):
    """A book entry. ``is_all``/``is_practice`` flag the client-side sentinels."""

    @property
    def book_id(self) -> Any:
        return self.raw.get("bookId")

    @property
    def book_name(self) -> Any:
        return self.raw.get("bookName")

    @property
    def is_all(self) -> bool:
        return self.book_id == "0"

    @property
    def is_practice(self) -> bool:
        return self.book_id == "WORK"


class WrongKnowledge(_Raw):
    """A knowledge-point row from ``wrong_knowledge``."""

    @property
    def qtype_id(self) -> Any:
        return self.raw.get("qTypeId")

    @property
    def qtype_name(self) -> Any:
        return self.raw.get("qTypeName")

    @property
    def knowledge_id(self) -> Any:
        return self.raw.get("knowledgeId")

    @property
    def knowledge_name(self) -> Any:
        return self.raw.get("knowledgeName")

    @property
    def wrong_times(self) -> Any:
        return self.raw.get("wrongTimes")

    @property
    def wrong_count(self) -> Any:
        return self.raw.get("wrongCount")


class WrongQuestion(_Raw):
    """A question entry (intentionally loose: only ``question_id`` is named)."""

    @property
    def question_id(self) -> Any:
        return self.raw.get("questionId")


class WrongNotePage:
    """One page of wrongnote rows plus its parsed pager.

    ``items`` holds model instances when an ``item_type`` is supplied to
    :meth:`from_dict` (e.g. a page of :class:`WrongKnowledge`), otherwise the
    raw dicts. ``raw`` is the page ``data`` object so nothing is lost.
    """

    def __init__(
        self,
        items: list[Any],
        current_page: Any,
        page_size: Any,
        page_count: Any,
        raw: Mapping[str, Any] | None = None,
        offset: Any = None,
    ):
        self.items = items
        self.current_page = current_page
        self.page_size = page_size
        self.page_count = page_count
        self.offset = offset
        self.raw = dict(raw) if raw is not None else {}

    @classmethod
    def from_dict(
        cls,
        data: Mapping[str, Any],
        *,
        item_type: type | None = None,
    ) -> "WrongNotePage":
        rows = data.get("list") or []
        pager = data.get("pager") or {}
        items = (
            [item_type.from_dict(row) for row in rows]
            if item_type is not None
            else list(rows)
        )
        return cls(
            items,
            pager.get("currentPage"),
            pager.get("pageSize"),
            pager.get("pageCount"),
            raw=data,
            offset=pager.get("offset"),
        )

    @property
    def has_next(self) -> bool:
        if self.current_page is None or self.page_count is None:
            return False
        return self.current_page < self.page_count

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)

    def __repr__(self) -> str:
        return (
            f"WrongNotePage(items={len(self.items)}, "
            f"current_page={self.current_page!r}, page_count={self.page_count!r})"
        )


class WrongSubjectStat(_Raw):
    """Aggregate counters from ``wrong_subject_stat``."""

    @property
    def wrong_count(self) -> Any:
        return self.raw.get("wrongCount")

    @property
    def killed_count(self) -> Any:
        return self.raw.get("killedCount")

    @property
    def killed_count_lately(self) -> Any:
        return self.raw.get("killedCountLately")


class WrongCourse(_Raw):
    """A course entry from ``wrong_courses``."""

    @property
    def course_id(self) -> Any:
        return self.raw.get("courseId")

    @property
    def course_name(self) -> Any:
        return self.raw.get("courseName")


class WrongRank(_Raw):
    """A rank row. The H5 table renders ``realname``/``wrongCount``/
    ``killedCount``/``recentKilledCount``; other fields stay in ``raw``."""

    @property
    def realname(self) -> Any:
        return self.raw.get("realname")

    @property
    def wrong_count(self) -> Any:
        return self.raw.get("wrongCount")

    @property
    def killed_count(self) -> Any:
        return self.raw.get("killedCount")

    @property
    def recent_killed_count(self) -> Any:
        return self.raw.get("recentKilledCount")


class WrongNoteAPI:
    """Wrong-answer book endpoints. Mixed into :class:`up366.client.Up366`."""

    @staticmethod
    def _time_value(value: Any) -> Any:
        """Match the H5 client: ``None`` becomes an empty form value."""
        return "" if value is None else value

    def _wrongnote_page(
        self,
        payload: dict,
        item_type: type | None = None,
    ) -> WrongNotePage:
        data = payload.get("data")
        if not data:
            # Empty dict / empty list / None all mean "no questions" in the
            # H5 client; e.g. kill/question/show returns [] for today.
            return WrongNotePage([], None, None, None, raw={})
        if not isinstance(data, dict):
            raise Up366ResponseError(
                "wrongnote response did not contain a data object"
            )
        self._page_data(payload)  # validate data.list / data.pager
        return WrongNotePage.from_dict(data, item_type=item_type)

    def _wrongnote_write(
        self,
        key: str,
        payload: Mapping[str, Any],
        *,
        dry_run: bool,
        confirm: bool,
    ) -> dict:
        if not isinstance(payload, Mapping) or not payload:
            raise ValueError("payload must be a non-empty mapping")
        endpoint = ENDPOINTS[key]
        url = endpoint.base_url + endpoint.path
        body = dict(payload)
        if dry_run or not confirm:
            return {"endpoint": key, "url": url, "data": body}
        return self._request("POST", url, data=body)

    # --- reads -----------------------------------------------------------

    def wrong_subjects(self) -> list[WrongSubject]:
        """Return the student's subjects (top-level array response)."""
        endpoint = ENDPOINTS["wrong_subjects"]
        payload = self._request(endpoint.method, endpoint.base_url + endpoint.path)
        return [WrongSubject.from_dict(row) for row in self._array_data(payload)]

    def wrong_subject_stat(
        self,
        *,
        book_id: Any = "0",
        stage_id: Any,
        subject_id: Any,
        start_time: Any = None,
        end_time: Any = None,
    ) -> WrongSubjectStat:
        """Return wrong/killed counters for one subject.

        Time values match the H5 client's ``YYYY-MM-DD 00:00:00`` /
        ``YYYY-MM-DD 23:59:59`` form; ``None`` is sent as an empty string.
        """
        endpoint = ENDPOINTS["wrong_subject_stat"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "bookId": book_id,
                "stageId": stage_id,
                "subjectId": subject_id,
                "startTime": self._time_value(start_time),
                "endTime": self._time_value(end_time),
            },
        )
        return WrongSubjectStat.from_dict(self._dict_data(payload))

    def wrong_books(self, *, stage_id: Any, subject_id: Any) -> list[WrongBook]:
        """Return the server's books plus the H5 UI's two sentinel entries.

        The server only returns real books; the H5 page prepends
        ``{bookId:"0", bookName:"全部"}`` and appends
        ``{bookId:"WORK", bookName:"练习"}``. This library mirrors that
        convenience so callers see the same list as the UI.
        """
        endpoint = ENDPOINTS["wrong_books"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"stageId": stage_id, "subjectId": subject_id},
        )
        rows = self._array_data(payload)
        synthetic = (
            [{"bookId": "0", "bookName": "全部"}]
            + list(rows)
            + [{"bookId": "WORK", "bookName": "练习"}]
        )
        return [WrongBook.from_dict(row) for row in synthetic]

    def wrong_knowledge(
        self,
        *,
        book_id: Any = "0",
        stage_id: Any,
        subject_id: Any,
        start_time: Any = None,
        end_time: Any = None,
        order_type: str = "desc",
        order_field: str = "wrongTimes",
        page: int = 1,
        page_size: int = 100,
    ) -> WrongNotePage:
        """Return a page of knowledge-point rows (page of ``WrongKnowledge``).

        ``order_field`` is one of the H5 table columns:
        ``qTypeName`` / ``knowledgeName`` / ``wrongTimes`` / ``wrongCount``.
        The H5 client sends the pager in both the form body and the query;
        this method does the same.
        """
        endpoint = ENDPOINTS["wrong_knowledge"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "bookId": book_id,
                "stageId": stage_id,
                "subjectId": subject_id,
                "startTime": self._time_value(start_time),
                "endTime": self._time_value(end_time),
                "orderType": order_type,
                "orderField": order_field,
                "pager###pageSize": page_size,
                "pager###currentPage": page,
            },
            params={
                "pager.currentPage": page,
                "pager.pageSize": page_size,
            },
        )
        return self._wrongnote_page(payload, WrongKnowledge)

    def wrong_courses(self, *, stage_id: Any, subject_id: Any) -> list[WrongCourse]:
        """Return the student's courses (top-level array response)."""
        endpoint = ENDPOINTS["wrong_courses"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={"stageId": stage_id, "subjectId": subject_id},
        )
        return [WrongCourse.from_dict(row) for row in self._array_data(payload)]

    def wrong_rank(
        self,
        *,
        course_id: Any,
        order_type: str = "desc",
        order_field: str = "wrongCount",
        page: int = 1,
        page_size: int = 100,
    ) -> WrongNotePage:
        """Return a rank page (page of ``WrongRank``).

        ``order_field`` is one of ``wrongCount`` / ``killedCount`` /
        ``recentKilledCount``. The H5 client sends the pager in both the form
        body and the query; this method does the same.
        """
        endpoint = ENDPOINTS["wrong_rank"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "courseId": course_id,
                "orderType": order_type,
                "orderField": order_field,
                "pager###pageSize": page_size,
                "pager###currentPage": page,
            },
            params={
                "pager.currentPage": page,
                "pager.pageSize": page_size,
            },
        )
        return self._wrongnote_page(payload, WrongRank)

    def wrong_killed_questions(
        self,
        *,
        book_id: Any,
        uid: Any,
        stage_id: Any,
        subject_id: Any,
        kill_flag: Any,
        start_time: Any = None,
        end_time: Any = None,
        page: int = 1,
        page_size: int = 10,
    ) -> WrongNotePage:
        """Return the killed-question paper (H5 ``kill/question/show``).

        ``kill_flag`` is 0 for the all-time list (the H5 client adds the date
        range only then) and 1 for today. Items are ``WrongQuestion`` rows.
        """
        endpoint = ENDPOINTS["wrong_killed_questions"]
        data: dict[str, Any] = {
            "bookId": book_id,
            "uid": uid,
            "stageId": stage_id,
            "subjectId": subject_id,
            "killFlag": kill_flag,
            "pager###pageSize": page_size,
            "pager###currentPage": page,
        }
        if str(kill_flag) == "0":
            data["startTime"] = self._time_value(start_time)
            data["endTime"] = self._time_value(end_time)
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data=data,
            params={
                "pager.currentPage": page,
                "pager.pageSize": page_size,
            },
        )
        return self._wrongnote_page(payload, WrongQuestion)

    def wrong_question_show(
        self,
        *,
        stage_id: Any,
        subject_id: Any,
        book_id: Any,
        qtype_id: Any,
        knowledge_id: Any,
        start_time: Any = None,
        end_time: Any = None,
        page: int = 1,
        page_size: int = 10,
    ) -> WrongNotePage:
        """Return the remaining-question paper for one knowledge point.

        H5 ``student/question/show``; items are ``WrongQuestion`` rows.
        """
        endpoint = ENDPOINTS["wrong_question_show"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "stageId": stage_id,
                "subjectId": subject_id,
                "bookId": book_id,
                "qtypeId": qtype_id,
                "knowledgeId": knowledge_id,
                "startTime": self._time_value(start_time),
                "endTime": self._time_value(end_time),
                "pager###pageSize": page_size,
                "pager###currentPage": page,
            },
            params={
                "pager.currentPage": page,
                "pager.pageSize": page_size,
            },
        )
        return self._wrongnote_page(payload, WrongQuestion)

    def wrong_kill_paper(
        self,
        *,
        book_id: Any,
        stage_id: Any,
        subject_id: Any,
        start_time: Any = None,
        end_time: Any = None,
    ) -> dict:
        """Return the paper a student must answer to kill wrong questions.

        H5 endpoint ``/client/wrongnote/student/questions/kill`` is a READ:
        the kill page fetches ``pathPrefix/paperData/answerData`` and renders
        it, then submits answers through :meth:`submit_wrong_question`.
        """
        endpoint = ENDPOINTS["wrong_kill_paper"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "bookId": book_id,
                "stageId": stage_id,
                "subjectId": subject_id,
                "startTime": self._time_value(start_time),
                "endTime": self._time_value(end_time),
            },
        )
        data = payload.get("data")
        if data in (None, {}, []):
            return {}
        return self._dict_data(payload)

    # --- writes (destructive, strictly gated) ----------------------------

    def submit_wrong_question(
        self,
        payload: Mapping[str, Any],
        *,
        dry_run: bool = True,
        confirm: bool = False,
    ) -> dict:
        """Submit killed-question answers (destructive, default dry-run).

        Payload (from the H5 ``question/submit`` call):

        ``uid``          student uid as a string
        ``result``       JSON string ``{"answerdata": <xml>,
                                          "mark_datao": "<elements>...</elements>"}``
        ``startTime``    epoch milliseconds as a string (server time at open)
        ``endTime``      epoch milliseconds as a string (server time at submit)
        ``seconds``      ``ceil((endTime-startTime)/1000)``
        ``rightQstIds``  comma-separated ids, or ``""``
        """
        return self._wrongnote_write(
            "submit_wrong_question", payload, dry_run=dry_run, confirm=confirm
        )
