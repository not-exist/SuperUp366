from __future__ import annotations

import json
from random import Random

import pytest

from up366 import Job
from up366_tui.wordpk import (
    bucket_id,
    select_word_pk_words,
    selection_rows,
    usable_words,
)


def _word(index: int) -> dict:
    return {
        "entryId": index,
        "entry": f"word{index}",
        "paraphrase": f"释义{index}",
    }


def _words(count: int = 10) -> list[dict]:
    return [_word(index) for index in range(1, count + 1)]


def test_select_word_pk_words_splits_by_score() -> None:
    selection = select_word_pk_words(
        _words(), score=7, total=10, rng=Random(42)
    )

    assert selection.score == 7
    assert selection.total == 10
    assert selection.right_count == 7
    assert selection.wrong_count == 3
    picked = [word["entryId"] for word in selection.correct + selection.wrong]
    assert len(picked) == 10
    assert len(set(picked)) == 10


def test_select_word_pk_words_is_reproducible() -> None:
    first = select_word_pk_words(_words(), score=4, total=6, rng=Random(7))
    second = select_word_pk_words(_words(), score=4, total=6, rng=Random(7))
    assert first.correct == second.correct
    assert first.wrong == second.wrong


def test_select_word_pk_words_allows_zero_and_full_score() -> None:
    zero = select_word_pk_words(_words(), score=0, total=5)
    assert zero.right_count == 0
    assert zero.wrong_count == 5

    full = select_word_pk_words(_words(), score=5, total=5)
    assert full.right_count == 5
    assert full.wrong_count == 0


@pytest.mark.parametrize(
    ("score", "total", "message"),
    [
        (5, 4, "分数不能大于 total"),
        (-1, 4, "分数不能为负数"),
        (1, 0, "total 必须大于 0"),
        (1, 11, "不足以抽取"),
    ],
)
def test_select_word_pk_words_rejects_bad_bounds(
    score: int, total: int, message: str
) -> None:
    with pytest.raises(ValueError, match=message):
        select_word_pk_words(_words(), score=score, total=total)


def test_usable_words_filters_and_deduplicates() -> None:
    words = [
        {"entryId": 1, "entry": "a"},
        {"pkid": 2, "entry": "b"},
        {"entry": "no-id"},
        "not-a-mapping",
        {"entryId": 1, "entry": "duplicate"},
        {"entryId": 3, "entry": "c"},
    ]

    usable = usable_words(words)

    assert [word["entry"] for word in usable] == ["a", "b", "c"]


def test_bucket_id_reads_xot_param() -> None:
    job = Job.from_dict(
        {
            "jobId": "JOB-1",
            "jobName": "Unit 4 单词PK",
            "courseId": 1,
            "extParam": json.dumps(
                {"xotParam": json.dumps({"pkBucketId": "BUCKET-1"})}
            ),
        }
    )
    assert bucket_id(job) == "BUCKET-1"

    ordinary = Job.from_dict(
        {
            "jobId": "JOB-2",
            "jobName": "普通作业",
            "courseId": 1,
            "extParam": json.dumps({"xotParam": json.dumps({"reportType": 0})}),
        }
    )
    assert bucket_id(ordinary) is None


def test_selection_rows_marks_correct_and_wrong() -> None:
    selection = select_word_pk_words(
        _words(3), score=2, total=3, rng=Random(0)
    )
    rows = selection_rows(selection)

    assert len(rows) == 3
    assert [row[3] for row in rows] == ["正确", "正确", "错误"]
    assert all(row[1].startswith("word") for row in rows)
    assert all(row[2].startswith("释义") for row in rows)
