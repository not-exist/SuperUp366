"""Ordinary book models and API mixin.

``BookAPI`` is mixed into :class:`~up366.client.Up366`; ``self`` is therefore an
authenticated session exposing ``_request`` and the ``_*_data`` helpers.
"""
from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from .endpoints import ENDPOINTS

__all__ = ["Book", "Chapter", "BookAPI"]


def _pick(raw: Mapping[str, Any], *keys: str) -> Any:
    """Return the first non-None value among ``keys`` (tolerant aliases)."""
    for key in keys:
        value = raw.get(key)
        if value is not None:
            return value
    return None


class Book:
    """A textbook/bookshelf entry (thin wrapper, like ``Job``).

    Bookshelf items and book-info responses have different shapes, so every
    field is read through tolerant aliases. ``book_id`` and ``goods_id`` are
    deliberately independent — a missing ``bookId`` never falls back to
    ``goodsId``.
    """

    def __init__(self, raw: Mapping[str, Any]):
        if not isinstance(raw, Mapping):
            raise TypeError("Book raw data must be a mapping")
        self.raw = dict(raw)

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "Book":
        return cls(raw)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)

    @property
    def book_id(self) -> Any:
        return _pick(self.raw, "bookId", "book_id")

    @property
    def goods_id(self) -> Any:
        return _pick(self.raw, "goodsId", "goods_id")

    @property
    def book_name(self) -> str:
        return _pick(self.raw, "bookName", "name", "book_name") or ""

    @property
    def cover(self) -> Any:
        return _pick(self.raw, "goodsImg", "coverImg", "cover", "bookImg")

    @property
    def vertical_cover(self) -> Any:
        return _pick(self.raw, "goodsVerticalImg", "verticalCover", "vertical_cover")

    @property
    def pack_type(self) -> Any:
        return _pick(self.raw, "packType", "pack_type")

    @property
    def is_2025_reader(self) -> bool:
        return self.pack_type == 9 or str(self.pack_type) == "9"

    @property
    def start_date(self) -> Any:
        return _pick(self.raw, "startDate", "start_date")

    @property
    def end_date(self) -> Any:
        return _pick(self.raw, "endDate", "end_date")

    @property
    def valid_days(self) -> Any:
        return _pick(self.raw, "validDays", "valid_days")

    @property
    def stage_id(self) -> Any:
        return _pick(self.raw, "stageId", "stage_id")

    @property
    def subject_id(self) -> Any:
        return _pick(self.raw, "subjectId", "subject_id")

    @property
    def category_type(self) -> Any:
        return _pick(self.raw, "categoryType", "category_type")

    @property
    def file_id(self) -> Any:
        return _pick(self.raw, "fileId", "file_id")

    @property
    def pc_file_id(self) -> Any:
        return _pick(self.raw, "pcFileId", "pc_file_id")

    @property
    def pc_version(self) -> Any:
        return _pick(self.raw, "pcVersion", "pc_version")

    @property
    def resource_list(self) -> Any:
        return _pick(self.raw, "resourceList", "resource_list")

    def __repr__(self) -> str:
        return f"Book(book_id={self.book_id!r}, name={self.book_name!r})"


class Chapter:
    """A chapter node (tree / custom / book-plan shape, thin wrapper)."""

    def __init__(self, raw: Mapping[str, Any]):
        if not isinstance(raw, Mapping):
            raise TypeError("Chapter raw data must be a mapping")
        self.raw = dict(raw)

    @classmethod
    def from_dict(cls, raw: Mapping[str, Any]) -> "Chapter":
        return cls(raw)

    def to_dict(self) -> dict[str, Any]:
        return dict(self.raw)

    @property
    def id(self) -> Any:
        return _pick(self.raw, "id", "chapterId", "chapter_id")

    @property
    def parent_id(self) -> Any:
        return _pick(self.raw, "parentId", "parent_id")

    @property
    def chapter_id(self) -> Any:
        return _pick(self.raw, "chapterId", "chapter_id", "id")

    @property
    def name(self) -> str:
        return _pick(self.raw, "name", "chapterName", "chapter_name") or ""

    @property
    def type(self) -> Any:
        return _pick(self.raw, "type", "chapterType")

    @property
    def is_close(self) -> Any:
        return _pick(self.raw, "isClose", "is_close")

    def __repr__(self) -> str:
        return f"Chapter(id={self.id!r}, chapter_id={self.chapter_id!r}, name={self.name!r})"


class BookAPI:
    """Read-only ordinary book endpoints (book-api / studytask-api)."""

    def books(self) -> list[Book]:
        """Return the current student's bookshelf (top-level array response)."""
        ep = ENDPOINTS["books"]
        payload = self._request(ep.method, ep.base_url + ep.path, data=None)
        return [Book.from_dict(item) for item in self._array_data(payload)]

    def book_info(self, book_id, *, original_book_id: str = "") -> Book:
        """Return one book's full metadata."""
        ep = ENDPOINTS["book_info"]
        payload = self._request(
            ep.method,
            ep.base_url + ep.path,
            data={
                "bookId": book_id,
                "bookType": 1,
                "originalBookId": original_book_id,
            },
        )
        return Book.from_dict(self._dict_data(payload))

    def book_tree(self, book_id, *, version: str = "") -> dict:
        """Return the raw chapter/page/task tree for a book.

        ``version=""`` forces a full refresh, so the key is always sent.
        """
        ep = ENDPOINTS["book_tree"]
        payload = self._request(
            ep.method,
            ep.base_url + ep.path,
            data={"bookId": book_id, "version": version, "bookType": 1},
        )
        return self._dict_data(payload)

    def custom_chapters(self, book_id, course_id) -> dict:
        """Return the custom chapter catalog for a book in a course."""
        ep = ENDPOINTS["custom_chapters"]
        payload = self._request(
            ep.method,
            ep.base_url + ep.path,
            data={"bookId": book_id, "courseId": course_id},
        )
        return self._dict_data(payload)

    def personalized_chapters(self, book_id) -> dict:
        """Return the personalized chapter set for a book."""
        ep = ENDPOINTS["personalized_chapters"]
        payload = self._request(
            ep.method,
            ep.base_url + ep.path,
            data={"bookId": book_id},
        )
        return self._dict_data(payload)

    def offline_books(self) -> list[str]:
        """Return the whitelist of book ids that may be cached offline."""
        ep = ENDPOINTS["offline_books"]
        payload = self._request(ep.method, ep.base_url + ep.path, data=None)
        book_ids = self._dict_data(payload).get("bookIds")
        return book_ids if isinstance(book_ids, list) else []

    def book_plan(self, course_id, book_id, *, book_type: int = 1) -> list[dict]:
        """Return the course study plan (chapter on/off switches)."""
        ep = ENDPOINTS["book_plan"]
        if course_id is None or course_id <= 0:
            return []
        payload = self._request(
            ep.method,
            ep.base_url + ep.path,
            data={"bookId": book_id, "courseId": course_id, "bookType": book_type},
        )
        return self._array_data(payload)
