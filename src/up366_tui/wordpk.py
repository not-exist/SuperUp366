"""Word-PK helpers: bucket lookup, random selection and preview rows."""
from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from random import Random
from typing import Any

from up366.words import word_cn


@dataclass(frozen=True)
class WordPkSelection:
    """One submitted round: ``score`` correct words and the rest wrong."""

    score: int
    total: int
    correct: tuple[dict[str, Any], ...]
    wrong: tuple[dict[str, Any], ...]

    @property
    def right_count(self) -> int:
        return len(self.correct)

    @property
    def wrong_count(self) -> int:
        return len(self.wrong)


def bucket_id(job: Any) -> str | None:
    """Return the word-PK bucket id from a :class:`up366.Job`."""
    value = job.xot_param.get("pkBucketId") if job is not None else None
    if value is None or value == "":
        return None
    return str(value)


def usable_words(words: Sequence[Mapping[str, Any]]) -> list[dict[str, Any]]:
    """Keep unique word entries that can be identified by the server.

    ``bucket_words`` normally returns dicts with ``entryId``; a few older
    buckets use ``pkid`` instead.  Entries without either id cannot be
    submitted, so they are dropped rather than producing a broken payload.
    """
    result: list[dict[str, Any]] = []
    seen: set[str] = set()
    for word in words:
        if not isinstance(word, Mapping):
            continue
        entry_id = word.get("entryId")
        if entry_id is None:
            entry_id = word.get("pkid")
        if entry_id is None or entry_id == "":
            continue
        key = str(entry_id)
        if key in seen:
            continue
        seen.add(key)
        result.append(dict(word))
    return result


def select_word_pk_words(
    words: Sequence[Mapping[str, Any]],
    *,
    score: int,
    total: int,
    rng: Random | None = None,
) -> WordPkSelection:
    """Randomly pick ``total`` words and split them by the requested score.

    Word-PK scoring is ``score = rightWordList.length`` (see the word-PK
    recon), so the requested score is also the number of correct words.
    """
    if total <= 0:
        raise ValueError("total 必须大于 0")
    if score < 0:
        raise ValueError("分数不能为负数")
    if score > total:
        raise ValueError("分数不能大于 total")
    pool = usable_words(words)
    if total > len(pool):
        raise ValueError(
            f"词库只有 {len(pool)} 个可用单词，不足以抽取 {total} 个"
        )
    picked = (rng or Random()).sample(pool, total)
    return WordPkSelection(
        score=score,
        total=total,
        correct=tuple(picked[:score]),
        wrong=tuple(picked[score:]),
    )


def selection_rows(
    selection: WordPkSelection,
    *,
    limit: int | None = None,
) -> list[tuple[str, str, str, str]]:
    """Return ``(index, word, translation, result)`` rows for preview."""
    rows: list[tuple[str, str, str, str]] = []
    ordered = list(selection.correct) + list(selection.wrong)
    for index, word in enumerate(ordered, 1):
        if limit is not None and index > limit:
            break
        rows.append(
            (
                str(index),
                str(word.get("entry") or ""),
                word_cn(word),
                "正确" if index <= selection.score else "错误",
            )
        )
    return rows
