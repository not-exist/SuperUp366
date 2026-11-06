from __future__ import annotations

import urllib.parse

import pytest

from up366_tui.paper import (
    parse_attachment,
    parse_paper_xml,
    question_type_name,
)


def _paper_xml(attachment: str = "") -> str:
    encoded = urllib.parse.quote_plus(attachment) if attachment else ""
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        "<elements>\n"
        '  <element id="Q1" type="3">\n'
        "    <question_no>1</question_no>\n"
        '    <question_text><![CDATA[<img src="media.jpg">Hello world.]]></question_text>\n'
        "    <question_score>10.0</question_score>\n"
        "    <question_type>9</question_type>\n"
        "    <duration>82</duration>\n"
        f"    <attachment><![CDATA[{encoded}]]></attachment>\n"
        "  </element>\n"
        '  <element id="Q2" type="2">\n'
        "    <question_no>2</question_no>\n"
        "    <question_score>5</question_score>\n"
        "    <question_type>4</question_type>\n"
        "    <duration>30</duration>\n"
        "  </element>\n"
        "</elements>\n"
    )


ATTACHMENT = (
    "<root><paragraph>"
    '<sentences id="1"><content><![CDATA[Hello world.]]></content>'
    "<content_cn><![CDATA[你好世界。]]></content_cn></sentences>"
    '<sentences id="2"><content><![CDATA[Second line.]]></content></sentences>'
    "</paragraph></root>"
)


def test_parse_paper_extracts_readaloud_questions() -> None:
    paper = parse_paper_xml(_paper_xml(ATTACHMENT))

    assert len(paper.questions) == 2
    assert paper.question_type_counts == {4: 1, 9: 1}

    readaloud = paper.readaloud_questions
    assert len(readaloud) == 1
    question = readaloud[0]
    assert question.element_id == "Q1"
    assert question.score == 10.0
    assert question.duration == 82
    assert question.sentence_count == 2
    assert question.sentences == ("Hello world.", "Second line.")
    assert question.preview == "Hello world."
    assert question.is_readaloud is True

    assert question_type_name(9) == "跟读"
    assert question_type_name(4) == "类型 4"


def test_parse_attachment_falls_back_to_regex() -> None:
    raw = "junk<content><![CDATA[One]]></content><content><![CDATA[Two]]></content>"
    assert parse_attachment(raw) == ("One", "Two")


def test_parse_paper_rejects_dtd() -> None:
    with pytest.raises(ValueError, match="DTD/ENTITY"):
        parse_paper_xml('<!DOCTYPE elements [<!ENTITY x "y">]><elements/>')


def test_empty_paper_has_no_questions() -> None:
    paper = parse_paper_xml("")
    assert paper.questions == []
    assert paper.readaloud_questions == []
    assert paper.question_type_counts == {}
