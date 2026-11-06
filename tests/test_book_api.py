from __future__ import annotations

import httpx
import pytest

import up366.client as client_module
from up366 import Up366, Up366ResponseError
from up366.book import Book, Chapter

BOOKS_URL = "https://book-api.up366.cn/client/books/buy-book"
BOOK_INFO_URL = "https://book-api.up366.cn/client/v2/book/info"
BOOK_TREE_URL = "https://book-api.up366.cn/client/books/tree"
CUSTOM_CHAPTERS_URL = "https://book-api.up366.cn/client/custom/book/chapters"
PERSONALIZED_CHAPTERS_URL = "https://book-api.up366.cn/client/chapters/personalized/list"
OFFLINE_BOOKS_URL = "https://book-api.up366.cn/front/offline/download/books"
BOOK_PLAN_URL = "https://studytask-api.up366.cn/client/book/plan"


def _ok(data) -> dict:
    return {"result": {"code": 0, "msg": "ok"}, "data": data}


def _response(payload: dict, method: str = "POST", url: str = "https://example.test") -> httpx.Response:
    return httpx.Response(
        200,
        json=payload,
        request=httpx.Request(method, url),
    )


def _session() -> Up366:
    return Up366(
        username="alice",
        password_hash="hash",
        tgt="TGT-123",
        client_id="CID-TEST",
        cookies=httpx.Cookies(),
    )


def test_books_posts_empty_body_and_returns_books(monkeypatch):
    captured: dict[str, object] = {}
    shelf = [
        {
            "bookId": "BOOK-1",
            "goodsId": 99001,
            "bookName": "英语必修一",
            "goodsImg": "https://cdn/cover.jpg",
            "goodsVerticalImg": "https://cdn/vertical.jpg",
            "packType": 9,
            "startDate": "2026-09-01",
            "endDate": "2027-07-01",
            "validDays": 300,
            "stageId": 2,
            "subjectId": 3,
            "categoryType": 1,
            "fileId": "F-1",
            "pcFileId": "PCF-1",
            "pcVersion": 5,
            "resourceList": {"dirChapters": []},
        },
    ]

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(_ok(shelf), method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().books()

    assert len(result) == 1
    book = result[0]
    assert isinstance(book, Book)
    assert book.book_id == "BOOK-1"
    assert book.goods_id == 99001
    assert book.book_name == "英语必修一"
    assert book.cover == "https://cdn/cover.jpg"
    assert book.vertical_cover == "https://cdn/vertical.jpg"
    assert book.pack_type == 9
    assert book.is_2025_reader is True
    assert book.start_date == "2026-09-01"
    assert book.end_date == "2027-07-01"
    assert book.valid_days == 300
    assert book.stage_id == 2
    assert book.subject_id == 3
    assert book.category_type == 1
    assert book.file_id == "F-1"
    assert book.pc_file_id == "PCF-1"
    assert book.pc_version == 5
    assert book.resource_list == {"dirChapters": []}
    assert book.raw == shelf[0]
    assert book.to_dict() == shelf[0]
    assert captured["method"] == "POST"
    assert captured["url"] == BOOKS_URL
    assert captured["data"] is None
    assert captured["timeout"] == 20.0
    assert captured["headers"]["u3t"] == "TGT-123"
    assert captured["headers"]["clientId"] == "CID-TEST"


def test_book_info_posts_exact_body_and_returns_book(monkeypatch):
    captured: dict[str, object] = {}
    info = {
        "bookId": "BOOK-2",
        "goodsId": 88002,
        "bookName": "数学",
        "packType": 7,
        "goodsImg": "x",
        "goodsVerticalImg": "y",
    }

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(_ok(info), method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().book_info("BOOK-2", original_book_id="ORIG-9")

    assert isinstance(result, Book)
    assert result.book_id == "BOOK-2"
    assert result.goods_id == 88002
    assert result.book_name == "数学"
    assert result.is_2025_reader is False
    assert captured["method"] == "POST"
    assert captured["url"] == BOOK_INFO_URL
    assert captured["data"] == {
        "bookId": "BOOK-2",
        "bookType": "1",
        "originalBookId": "ORIG-9",
    }
    assert captured["headers"]["u3t"] == "TGT-123"


def test_book_tree_always_sends_version_even_when_empty(monkeypatch):
    captured: dict[str, object] = {}
    tree = {"version": "v42", "chapters": [], "pages": [], "tasks": [], "book": {}}

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(_ok(tree), method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    # default version="" must still be sent (force full refresh)
    result = _session().book_tree("BOOK-3")

    assert result == tree
    assert captured["method"] == "POST"
    assert captured["url"] == BOOK_TREE_URL
    assert captured["data"] == {"bookId": "BOOK-3", "version": "", "bookType": "1"}
    assert "version" in captured["data"]
    assert captured["headers"]["u3t"] == "TGT-123"

    assert _session().book_tree("BOOK-3", version="v42") == tree
    assert captured["data"] == {"bookId": "BOOK-3", "version": "v42", "bookType": "1"}


@pytest.mark.parametrize(
    "response_data, expected",
    [
        ({"bookIds": ["B1", "B2"]}, ["B1", "B2"]),
        ({"bookIds": []}, []),
        ({"bookIds": "not-a-list"}, []),
        ({"bookIds": None}, []),
        ({}, []),
    ],
)
def test_offline_books_returns_only_list_book_ids(monkeypatch, response_data, expected):
    captured: dict[str, object] = {}

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(_ok(response_data), method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().offline_books()

    assert result == expected
    assert captured["method"] == "POST"
    assert captured["url"] == OFFLINE_BOOKS_URL
    assert captured["data"] is None
    assert captured["headers"]["u3t"] == "TGT-123"


def test_book_plan_posts_expected_body_and_host(monkeypatch):
    captured: dict[str, object] = {}
    plan = [{"chapterId": "C1", "isClose": False}]

    def fake_request(method, url, *, data, headers, timeout, cookies):
        captured.update(method=method, url=url, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(_ok(plan), method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().book_plan(9222783, "BOOK-5")

    assert result == plan
    assert captured["method"] == "POST"
    assert captured["url"] == BOOK_PLAN_URL
    # course_id is the first positional arg, book_id the second
    assert captured["data"] == {
        "bookId": "BOOK-5",
        "courseId": "9222783",
        "bookType": "1",
    }
    assert list(captured["data"]) == ["bookId", "courseId", "bookType"]
    assert captured["headers"]["u3t"] == "TGT-123"


@pytest.mark.parametrize("course_id", [None, 0, -1])
def test_book_plan_skips_request_when_course_id_missing_or_nonpositive(monkeypatch, course_id):
    called = False

    def fake_request(method, url, *, data, headers, timeout, cookies):
        nonlocal called
        called = True
        return _response(_ok([]), method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = _session().book_plan(course_id, "BOOK-5")

    assert result == []
    assert called is False


@pytest.mark.parametrize(
    "method_name, args, kwargs, url, expected_data, response_data",
    [
        (
            "custom_chapters",
            ("BOOK-6", 9222783),
            {},
            CUSTOM_CHAPTERS_URL,
            {"bookId": "BOOK-6", "courseId": "9222783"},
            {"chapters": [{"chapterId": "C1"}]},
        ),
        (
            "personalized_chapters",
            ("BOOK-7",),
            {},
            PERSONALIZED_CHAPTERS_URL,
            {"bookId": "BOOK-7"},
            {"chapters": []},
        ),
        (
            "book_info",
            ("BOOK-8",),
            {},
            BOOK_INFO_URL,
            {"bookId": "BOOK-8", "bookType": "1", "originalBookId": ""},
            {"bookId": "BOOK-8"},
        ),
        (
            "book_plan",
            (9222783, "BOOK-9"),
            {"book_type": 2},
            BOOK_PLAN_URL,
            {"bookId": "BOOK-9", "courseId": "9222783", "bookType": "2"},
            [{"chapterId": "C9", "isClose": True}],
        ),
    ],
)
def test_remaining_post_bodies(monkeypatch, method_name, args, kwargs, url, expected_data, response_data):
    captured: dict[str, object] = {}

    def fake_request(method, rurl, *, data, headers, timeout, cookies):
        captured.update(method=method, url=rurl, data=data, headers=headers, timeout=timeout, cookies=cookies)
        return _response(_ok(response_data), method=method, url=rurl)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    result = getattr(_session(), method_name)(*args, **kwargs)

    if method_name == "book_info":
        assert isinstance(result, Book)
        assert result.book_id == "BOOK-8"
    else:
        assert result == response_data
    assert captured["method"] == "POST"
    assert captured["url"] == url
    assert captured["data"] == expected_data
    assert captured["headers"]["u3t"] == "TGT-123"


@pytest.mark.parametrize(
    "method_name, args, bad_data",
    [
        ("books", (), {"list": []}),
        ("book_info", ("B",), ["not-a-dict"]),
        ("book_tree", ("B",), "bad"),
        ("offline_books", (), ["not-a-dict"]),
        ("book_plan", (9222783, "B"), {"a": 1}),
    ],
)
def test_malformed_data_raises_response_error(monkeypatch, method_name, args, bad_data):
    def fake_request(method, url, *, data, headers, timeout, cookies):
        return _response(_ok(bad_data), method=method, url=url)

    monkeypatch.setattr(client_module.httpx, "request", fake_request)

    with pytest.raises(Up366ResponseError):
        getattr(_session(), method_name)(*args)


def test_book_and_chapter_models_are_tolerant():
    book = Book.from_dict(
        {"book_id": "B-1", "goods_id": 7, "book_name": "X", "pack_type": "9"}
    )
    assert book.book_id == "B-1"
    assert book.goods_id == 7
    assert book.book_name == "X"
    assert book.is_2025_reader is True
    assert book.to_dict()["book_id"] == "B-1"

    # book_id and goods_id stay separate: a missing bookId never falls back to goodsId
    only_goods = Book.from_dict({"goodsId": 7})
    assert only_goods.book_id is None
    assert only_goods.goods_id == 7

    chapter = Chapter.from_dict(
        {
            "id": 1,
            "parentId": 0,
            "chapterId": "C1",
            "name": "Unit 1",
            "type": 2,
            "isClose": True,
        }
    )
    assert chapter.id == 1
    assert chapter.parent_id == 0
    assert chapter.chapter_id == "C1"
    assert chapter.name == "Unit 1"
    assert chapter.type == 2
    assert chapter.is_close is True
    assert chapter.to_dict()["chapterId"] == "C1"
