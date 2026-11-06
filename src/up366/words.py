from __future__ import annotations

import time
from collections.abc import Mapping, Sequence
from typing import Any

__all__ = [
    "AUDIO_BASE",
    "build_word_info",
    "build_word_infos",
    "word_audio",
    "word_cn",
]

AUDIO_BASE = "https://word-cdn-alyun.up366.cn"


def word_cn(word: Mapping[str, Any]) -> str:
    """Return the display translation, preferring ``newPropList``."""
    parts: list[str] = []
    for prop in word.get("newPropList") or []:
        text = str(prop.get("prop") or "")
        has_sense = False
        for sense in prop.get("senseList") or []:
            if sense.get("senseType") == 0:
                text += str(sense.get("paraphrase") or "")
                has_sense = True
        if has_sense:
            parts.append(text)
    return "；".join(parts) if parts else str(word.get("paraphrase") or "")


def word_audio(word: Mapping[str, Any], base: str = AUDIO_BASE) -> str:
    """Return the word's audio URL, preferring the US voice file."""
    name = word.get("usVoiceFile") or word.get("enVoiceFile") or ""
    if not name:
        return ""
    if name.startswith("http"):
        return name
    return f"{base}/{name}"


def build_word_info(
    word: Mapping[str, Any],
    *,
    answer_status: int,
    index: int,
    answer: str | None = None,
    answer_time: int | None = None,
) -> dict[str, Any]:
    """Build one full word record for a word-PK submission."""
    return {
        "entryId": word.get("entryId"),
        "en": word.get("entry") or "",
        "cn": word_cn(word),
        "phonetic": word.get("usPhonetic") or word.get("enPhonetic") or "",
        "audio": word_audio(word),
        "sourceType": word.get("sourceType", 1),
        "localId": index,
        "answer": (
            answer
            if answer is not None
            else (word.get("entry") or "" if answer_status == 1 else "")
        ),
        "answerStatus": answer_status,
        "answerTime": (
            answer_time
            if answer_time is not None
            else int(time.time() * 1000) + index
        ),
    }


def build_word_infos(
    correct_words: Sequence[Mapping[str, Any]],
    wrong_words: Sequence[Mapping[str, Any]],
    *,
    answer_time: int | None = None,
) -> list[dict[str, Any]]:
    """Build correct records first, then wrong records, with sequential ids."""
    infos: list[dict[str, Any]] = []
    for answer_status, words in ((1, correct_words), (0, wrong_words)):
        for word in words:
            infos.append(
                build_word_info(
                    word,
                    answer_status=answer_status,
                    index=len(infos),
                    answer_time=answer_time,
                )
            )
    return infos
