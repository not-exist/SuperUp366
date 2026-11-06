from __future__ import annotations

import up366.words as words_module
from up366.words import AUDIO_BASE, build_word_info, build_word_infos, word_audio, word_cn


def _bucket_word(**overrides):
    word = {
        "entryId": 7465,
        "entry": "regardless of",
        "paraphrase": "Phrase不管；不顾",
        "usPhonetic": "",
        "enPhonetic": "[rɪˈɡɑːdləs əv]",
        "usVoiceFile": "entryfilesv2/7465/9F2D8C.mp3",
        "sourceType": 1,
        "newPropList": [],
    }
    word.update(overrides)
    return word


def test_build_word_info_matches_client_display_shape():
    info = build_word_info(
        _bucket_word(),
        answer_status=1,
        index=3,
        answer_time=1792141833479,
    )

    assert info["entryId"] == 7465
    assert info["en"] == "regardless of"
    assert info["cn"] == "Phrase不管；不顾"
    assert info["phonetic"] == "[rɪˈɡɑːdləs əv]"
    assert info["audio"] == (
        "https://word-cdn-alyun.up366.cn/entryfilesv2/7465/9F2D8C.mp3"
    )
    assert info["sourceType"] == 1
    assert info["localId"] == 3
    assert info["answer"] == "regardless of"
    assert info["answerStatus"] == 1
    assert info["answerTime"] == 1792141833479


def test_build_word_info_prefers_us_phonetic_and_defaults_source_type():
    word = _bucket_word(usPhonetic="[rɪˈɡɑːdləs əv]")
    word.pop("sourceType")

    info = build_word_info(word, answer_status=0, index=0, answer_time=1)

    assert info["phonetic"] == "[rɪˈɡɑːdləs əv]"
    assert info["sourceType"] == 1
    assert info["answer"] == ""


def test_build_word_info_accepts_explicit_answer():
    info = build_word_info(
        _bucket_word(),
        answer_status=0,
        index=1,
        answer="wrong attempt",
        answer_time=2,
    )

    assert info["answer"] == "wrong attempt"
    assert info["answerStatus"] == 0


def test_build_word_info_defaults_answer_time_to_now_plus_index(monkeypatch):
    monkeypatch.setattr(words_module.time, "time", lambda: 1_700_000_000.0)

    info = build_word_info(_bucket_word(), answer_status=1, index=7)

    assert info["answerTime"] == 1_700_000_000_000 + 7


def test_word_cn_joins_new_prop_list_senses():
    word = _bucket_word(
        newPropList=[
            {
                "prop": "n. ",
                "senseList": [
                    {"senseType": 0, "paraphrase": "自信，信心"},
                    {"senseType": 1, "paraphrase": "不进入释义"},
                ],
            },
            {
                "prop": "v. ",
                "senseList": [{"senseType": 0, "paraphrase": "使确信"}],
            },
        ]
    )

    assert word_cn(word) == "n. 自信，信心；v. 使确信"


def test_word_cn_falls_back_to_paraphrase():
    assert word_cn(_bucket_word()) == "Phrase不管；不顾"
    assert word_cn({"paraphrase": "n. 试验"}) == "n. 试验"


def test_word_cn_skips_prop_without_sense_type_zero():
    word = _bucket_word(
        newPropList=[
            {
                "prop": "Phrase",
                "senseList": [{"senseType": 1, "paraphrase": "不是释义"}],
            }
        ]
    )

    assert word_cn(word) == "Phrase不管；不顾"


def test_word_audio_prefixes_relative_us_voice_with_base():
    assert word_audio(_bucket_word()) == (
        AUDIO_BASE + "/entryfilesv2/7465/9F2D8C.mp3"
    )


def test_word_audio_falls_back_to_en_voice_and_custom_base():
    word = _bucket_word(
        usVoiceFile="",
        enVoiceFile="entryfilesv2/7465/en.mp3",
    )

    assert word_audio(word, "https://cdn.example.test") == (
        "https://cdn.example.test/entryfilesv2/7465/en.mp3"
    )


def test_word_audio_keeps_absolute_url():
    word = _bucket_word(usVoiceFile="https://cdn.example.test/7465.mp3")

    assert word_audio(word) == "https://cdn.example.test/7465.mp3"


def test_word_audio_returns_empty_without_voice_files():
    assert word_audio(_bucket_word(usVoiceFile="", enVoiceFile="")) == ""


def test_build_word_infos_orders_correct_then_wrong_with_consecutive_indices():
    correct = [
        _bucket_word(entryId=1, entry="apple"),
        _bucket_word(entryId=2, entry="banana"),
    ]
    wrong = [_bucket_word(entryId=3, entry="pear")]

    infos = build_word_infos(correct, wrong, answer_time=5000)

    assert [info["answerStatus"] for info in infos] == [1, 1, 0]
    assert [info["localId"] for info in infos] == [0, 1, 2]
    assert [info["entryId"] for info in infos] == [1, 2, 3]
    assert [info["answer"] for info in infos] == ["apple", "banana", ""]
    assert all(info["answerTime"] == 5000 for info in infos)
