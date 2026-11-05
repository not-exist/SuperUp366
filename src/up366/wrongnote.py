"""Wrong-answer book (``wrongnote-api``) models and API mixin.

Host and transport
------------------
These endpoints live on ``https://wrongnote-api.up366.cn`` — a *different*
host from the ordinary book APIs (``book-api`` / ``studytask-api``). They are
not merged into those mixins on purpose. All ten endpoints are POST.

Auth caveat (UNVERIFIED)
------------------------
The catalog was recovered from the H5 web SPA (``work/h5/js_chunk-*.js``),
where these endpoints are called with cookie / service-ticket auth.
``Up366._request`` instead sends the PC student-client ``u3t``/``u3r`` headers.
Whether the wrongnote host accepts PC auth has NOT been confirmed with a live
capture. This is documented here and deliberately NOT worked around in code;
a real captured request is needed before trusting these against production.

Writes
------
``kill_wrong_questions`` and ``submit_wrong_question`` are destructive and
irreversible. They default to ``dry_run=True`` (zero network calls, returns a
request preview) and require BOTH ``dry_run=False`` AND ``confirm=True`` to
actually send. They never auto-retry.

Experimental
------------
``wrong_killed_questions`` and ``wrong_question_show`` have unverified payload
shapes; their ``**params`` are passed straight through as the request body.
"""
from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from .endpoints import ENDPOINTS

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
    ):
        self.items = items
        self.current_page = current_page
        self.page_size = page_size
        self.page_count = page_count
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
        )

    @property
    def has_next(self) -> bool:
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
    """A rank row (loose wrapper; ``degree`` kept alongside the raw dict)."""

    @property
    def wrong_count(self) -> Any:
        return self.raw.get("wrongCount")

    @property
    def degree(self) -> Any:
        return self.raw.get("degree")


class WrongNoteAPI:
    """Wrong-answer book endpoints. Mixed into :class:`up366.client.Up366`."""

    def _wrongnote_page(
        self,
        payload: dict,
        item_type: type | None = None,
    ) -> WrongNotePage:
        items, pager = self._page_data(payload)
        return WrongNotePage.from_dict(
            {"list": items, "pager": pager}, item_type=item_type
        )

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
        """Return wrong/killed counters for one subject."""
        endpoint = ENDPOINTS["wrong_subject_stat"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "bookId": book_id,
                "stageId": stage_id,
                "subjectId": subject_id,
                "startTime": start_time,
                "endTime": end_time,
            },
        )
        return WrongSubjectStat.from_dict(self._dict_data(payload))

    def wrong_books(self, *, stage_id: Any, subject_id: Any) -> list[WrongBook]:
        """Return books with client-side ``全部``/``练习`` sentinels added."""
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
        """Return a page of knowledge-point rows (page of ``WrongKnowledge``)."""
        endpoint = ENDPOINTS["wrong_knowledge"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data={
                "bookId": book_id,
                "stageId": stage_id,
                "subjectId": subject_id,
                "startTime": start_time,
                "endTime": end_time,
                "orderType": order_type,
                "orderField": order_field,
                "pager###pageSize": page_size,
                "pager###currentPage": page,
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
        """Return a rank page (page of ``WrongRank``)."""
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
        )
        return self._wrongnote_page(payload, WrongRank)

    def wrong_killed_questions(self, **params: Any) -> WrongNotePage:
        """EXPERIMENTAL: payload shape unverified; ``params`` passed through.

        Recovered from the H5 SPA but not yet confirmed against a live capture.
        Returns a page of raw dicts.
        """
        endpoint = ENDPOINTS["wrong_killed_questions"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data=dict(params),
        )
        return self._wrongnote_page(payload)

    def wrong_question_show(self, **params: Any) -> WrongNotePage:
        """EXPERIMENTAL: payload shape unverified; ``params`` passed through.

        Recovered from the H5 SPA but not yet confirmed against a live capture.
        Returns a page of raw dicts.
        """
        endpoint = ENDPOINTS["wrong_question_show"]
        payload = self._request(
            endpoint.method,
            endpoint.base_url + endpoint.path,
            data=dict(params),
        )
        return self._wrongnote_page(payload)

    # --- writes (destructive, strictly gated) ----------------------------

    def kill_wrong_questions(
        self,
        payload: Mapping[str, Any],
        *,
        dry_run: bool = True,
        confirm: bool = False,
    ) -> dict:
        """Mark wrong questions as killed (destructive, default dry-run)."""
        return self._wrongnote_write(
            "kill_wrong_questions", payload, dry_run=dry_run, confirm=confirm
        )

    def submit_wrong_question(
        self,
        payload: Mapping[str, Any],
        *,
        dry_run: bool = True,
        confirm: bool = False,
    ) -> dict:
        """Submit a wrong question (destructive, default dry-run)."""
        return self._wrongnote_write(
            "submit_wrong_question", payload, dry_run=dry_run, confirm=confirm
        )
