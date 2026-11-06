"""Read-only parsing of up366 paper XML.

``paper_detail`` (qlib-api) and the downloaded ``paper.xml`` both use the
same ``<elements><element ...>`` shape.  A read-aloud question has
``question_type = 9`` and its ``<attachment>`` holds a URL-encoded XML
document with one ``<sentences>`` node per sentence.

The parser tolerates a missing or malformed ``<attachment>``: the question
count is still useful for deciding how many recordings a job needs.
"""
from __future__ import annotations

import html
import re
import urllib.parse
import xml.etree.ElementTree as ET
from dataclasses import dataclass, field

try:  # Optional hardening; the stdlib fallback below is guarded as well.
    from defusedxml import ElementTree as _SafeET
except ImportError:  # pragma: no cover - exercised when the extra is absent
    _SafeET = None

READALOUD_QUESTION_TYPE = 9
MAX_XML_CHARS = 8 * 1024 * 1024
_FORBIDDEN_XML = re.compile(r"<!\s*(?:DOCTYPE|ENTITY)", re.IGNORECASE)
SPOKEN_QUESTION_TYPE_NAMES = {
    8: "连线/匹配",
    9: "跟读",
    12: "口语问答",
    13: "口语复述",
}


@dataclass(frozen=True)
class Question:
    element_id: str
    question_type: int | None
    question_no: str
    score: float | None
    duration: int | None
    text: str
    sentences: tuple[str, ...] = ()

    @property
    def is_readaloud(self) -> bool:
        return self.question_type == READALOUD_QUESTION_TYPE

    @property
    def preview(self) -> str:
        return plain_text(self.text) or " ".join(self.sentences)

    @property
    def sentence_count(self) -> int:
        return len(self.sentences)


@dataclass
class Paper:
    questions: list[Question] = field(default_factory=list)

    @property
    def readaloud_questions(self) -> list[Question]:
        return [question for question in self.questions if question.is_readaloud]

    @property
    def question_type_counts(self) -> dict[int, int]:
        counts: dict[int, int] = {}
        for question in self.questions:
            if question.question_type is None:
                continue
            counts[question.question_type] = counts.get(question.question_type, 0) + 1
        return dict(sorted(counts.items()))


def question_type_name(question_type: int | None) -> str:
    if question_type is None:
        return "未知"
    return SPOKEN_QUESTION_TYPE_NAMES.get(question_type, f"类型 {question_type}")


def plain_text(value: str) -> str:
    """Strip editor HTML from ``question_text`` and collapse whitespace."""
    without_tags = re.sub(r"<[^>]*>", " ", value or "")
    return re.sub(r"\s+", " ", html.unescape(without_tags)).strip()


def _parse_xml(text: str) -> ET.Element:
    """Parse server-provided XML with entity/DTD and size guards.

    ``defusedxml`` is used when installed; otherwise a plain-text check
    rejects DTD/entity declarations before the stdlib parser sees them.
    """
    if len(text) > MAX_XML_CHARS:
        raise ValueError("XML 文档过大，已拒绝解析。")
    if _FORBIDDEN_XML.search(text):
        raise ValueError("XML 中包含不支持的 DTD/ENTITY 声明。")
    if _SafeET is not None:
        return _SafeET.fromstring(text)
    return ET.fromstring(text)


def _child_text(element: ET.Element, tag: str) -> str:
    child = element.find(tag)
    if child is None or child.text is None:
        return ""
    return child.text.strip()


def _to_float(value: str) -> float | None:
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _to_int(value: str) -> int | None:
    try:
        return int(float(value))
    except (TypeError, ValueError):
        return None


def parse_attachment(value: str) -> tuple[str, ...]:
    """Decode a question ``attachment`` and return its sentence texts."""
    value = (value or "").strip()
    if not value:
        return ()
    decoded = urllib.parse.unquote_plus(value)
    if "<" not in decoded:
        decoded = html.unescape(value)
    else:
        decoded = html.unescape(decoded)

    try:
        root = _parse_xml(decoded)
    except Exception:
        return tuple(
            match.strip()
            for match in re.findall(
                r"<content>\s*<!\[CDATA\[(.*?)\]\]>\s*</content>", decoded, re.S
            )
            if match.strip()
        )

    sentences: list[str] = []
    for node in root.iter("sentences"):
        content = node.find("content")
        if content is not None and content.text and content.text.strip():
            sentences.append(content.text.strip())
    if not sentences:
        for node in root.iter("content"):
            if node.text and node.text.strip():
                sentences.append(node.text.strip())
    return tuple(sentences)


def parse_paper_xml(xml_text: str) -> Paper:
    """Parse a paper XML document into :class:`Question` records.

    ``ValueError`` is raised only when the document cannot be parsed at all;
    an empty document yields an empty :class:`Paper`.
    """
    if not (xml_text or "").strip():
        return Paper()
    try:
        root = _parse_xml(xml_text)
    except Exception as exc:
        raise ValueError(f"试卷 XML 解析失败：{exc}") from exc

    if root.tag == "elements":
        container = root
    else:
        container = root.find(".//elements")
        if container is None:
            container = root

    elements = container.findall("element")
    if not elements:
        elements = root.findall(".//element")

    questions: list[Question] = []
    for element in elements:
        type_text = _child_text(element, "question_type")
        if not type_text:
            # A composite element can wrap child elements; only direct
            # question records carry their own question_type.
            continue
        questions.append(
            Question(
                element_id=element.get("id") or "",
                question_type=_to_int(type_text),
                question_no=_child_text(element, "question_no"),
                score=_to_float(_child_text(element, "question_score")),
                duration=_to_int(_child_text(element, "duration")),
                text=_child_text(element, "question_text"),
                sentences=parse_attachment(_child_text(element, "attachment")),
            )
        )
    return Paper(questions)
