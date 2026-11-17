//! Shared scanning of the `<elements>` paper XML.
//!
//! `/client/paper/detail` and `/client/textbook/paperinfo` both return the same
//! document shape: `<element>` nodes whose children carry the question type,
//! score, text and attachment. The read-aloud and applied-writing adapters
//! classify those nodes differently, so the raw scan lives here once and each
//! adapter decides what a question means.

use percent_encoding::percent_decode_str;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::{ProtocolError, Result};

/// One `<element>` before any adapter-specific classification.
#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct RawQuestion {
    pub question_id: String,
    /// `<element type="…">`: `3` denotes a question; 0/1/2 are layout.
    pub element_type: String,
    /// `<question_no>` as the paper spells it.
    pub question_no: String,
    /// `<question_type>` as text: `9`, `14`, `99`, ...
    pub question_type: String,
    pub score: f64,
    pub duration: u32,
    /// Decoded `<attachment>` document.
    pub attachment: String,
    /// Raw `<question_text>` HTML.
    pub question_text: String,
    /// `<options><wordnum>` for writing questions.
    pub word_num: Option<u32>,
}

/// Scan every `<element>` of a paper.
pub(crate) fn parse_questions(xml: &str) -> Result<Vec<RawQuestion>> {
    let mut questions = Vec::new();
    if xml.trim().is_empty() {
        return Ok(questions);
    }

    let mut reader = Reader::from_str(xml);
    let mut current: Option<RawQuestion> = None;
    let mut depth = 0usize;
    let mut collecting = Collecting::Nothing;
    let mut buffer = String::new();

    loop {
        let event = reader
            .read_event()
            .map_err(|error| ProtocolError::Decode(format!("paper XML is malformed: {error}")))?;
        if matches!(event, Event::Eof) {
            break;
        }
        match event {
            Event::Empty(start) if local_name(&start) == "element" && current.is_none() => {
                questions.push(RawQuestion {
                    question_id: attribute(&start, "id").unwrap_or_default(),
                    element_type: attribute(&start, "type").unwrap_or_default(),
                    ..Default::default()
                });
            }
            Event::Start(start) => {
                if local_name(&start) == "element" {
                    if current.is_none() {
                        current = Some(RawQuestion {
                            question_id: attribute(&start, "id").unwrap_or_default(),
                            element_type: attribute(&start, "type").unwrap_or_default(),
                            ..Default::default()
                        });
                    }
                    depth += 1;
                    continue;
                }
                if current.is_none() {
                    continue;
                }
                let next = match local_name(&start) {
                    "question_type" => Collecting::Type,
                    "question_score" => Collecting::Score,
                    "duration" => Collecting::Duration,
                    "attachment" => Collecting::Attachment,
                    "question_text" => Collecting::Text,
                    "question_no" => Collecting::QuestionNo,
                    "wordnum" => Collecting::WordNum,
                    _ => Collecting::Nothing,
                };
                if next != Collecting::Nothing {
                    collecting = next;
                    buffer.clear();
                }
            }
            Event::End(end) => {
                if end.name().local_name().into_inner() == "element" {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        if let Some(question) = current.take() {
                            questions.push(question);
                        }
                        collecting = Collecting::Nothing;
                    }
                    continue;
                }
                if let Some(question) = current.as_mut() {
                    match collecting {
                        Collecting::Type => question.question_type = buffer.trim().to_owned(),
                        Collecting::Score => {
                            question.score = buffer.trim().parse().unwrap_or_default()
                        }
                        Collecting::Duration => {
                            question.duration = buffer.trim().parse().unwrap_or_default()
                        }
                        Collecting::Attachment => {
                            question.attachment = decode_attachment(buffer.trim())
                        }
                        Collecting::Text => question.question_text = buffer.clone(),
                        Collecting::QuestionNo => question.question_no = buffer.trim().to_owned(),
                        Collecting::WordNum => question.word_num = buffer.trim().parse().ok(),
                        Collecting::Nothing => {}
                    }
                }
                collecting = Collecting::Nothing;
            }
            Event::Text(text) if collecting != Collecting::Nothing => buffer.push_str(&text),
            Event::CData(text) if collecting != Collecting::Nothing => buffer.push_str(&text),
            _ => {}
        }
    }
    if depth != 0 || current.is_some() {
        return Err(ProtocolError::Decode(
            "paper XML ended before all elements closed".to_owned(),
        ));
    }
    Ok(questions)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Collecting {
    Nothing,
    Type,
    Score,
    Duration,
    Attachment,
    Text,
    QuestionNo,
    WordNum,
}

/// `<attachment>` is URL-encoded; `+` means a space, as in a query string.
pub(crate) fn decode_attachment(encoded: &str) -> String {
    let with_spaces = encoded.replace('+', " ");
    percent_decode_str(&with_spaces)
        .decode_utf8_lossy()
        .into_owned()
}

/// Turn the `<question_text>` HTML into readable text.
///
/// Block tags become line breaks so a writing prompt keeps its paragraphs;
/// inline tags become spaces. Entity decoding is deliberately small: the
/// upstream papers only use the handful handled here.
pub(crate) fn plain_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for character in html.chars() {
        match character {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' => {
                in_tag = false;
                let name = tag
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if is_block_tag(&name) {
                    text.push('\n');
                } else {
                    text.push(' ');
                }
            }
            _ if !in_tag => text.push(character),
            _ => tag.push(character),
        }
    }
    let decoded = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    let mut lines = Vec::new();
    for line in decoded.lines() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines.join("\n")
}

fn is_block_tag(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "br"
            | "li"
            | "tr"
            | "table"
            | "ul"
            | "ol"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
    )
}

pub(crate) fn local_name<'a>(start: &'a BytesStart<'_>) -> &'a str {
    start.name().local_name().into_inner()
}

pub(crate) fn attribute(start: &BytesStart<'_>, name: &str) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().into_inner() == name)
        .and_then(|attribute| attribute.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|value| value.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_type_no_score_text_and_word_count() {
        let xml = r#"<elements>
            <element id="Q1" type="3">
                <question_no>2</question_no>
                <question_text><![CDATA[<p>Write a letter.</p><p>80 words.</p>]]></question_text>
                <options><wordnum>80</wordnum></options>
                <question_score>15.0</question_score>
                <question_type>14</question_type>
                <duration>1200</duration>
            </element>
        </elements>"#;
        let question = &parse_questions(xml).unwrap()[0];
        assert_eq!(question.question_id, "Q1");
        assert_eq!(question.element_type, "3");
        assert_eq!(question.question_no, "2");
        assert_eq!(question.question_type, "14");
        assert_eq!(question.score, 15.0);
        assert_eq!(question.duration, 1200);
        assert_eq!(question.word_num, Some(80));
        assert_eq!(
            plain_text(&question.question_text),
            "Write a letter.\n80 words."
        );
    }

    #[test]
    fn rejects_malformed_and_truncated_papers() {
        assert!(parse_questions(r#"<elements><element id="Q1">"#).is_err());
        assert!(parse_questions(r#"<elements><element id="Q1"></elements>"#).is_err());
    }

    #[test]
    fn keeps_self_closing_question_elements() {
        let questions =
            parse_questions(r#"<elements><element id="Q1" type="3"/></elements>"#).unwrap();
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].question_id, "Q1");
        assert_eq!(questions[0].element_type, "3");
    }

    #[test]
    fn decodes_a_url_encoded_attachment() {
        assert_eq!(
            decode_attachment(
                "%3Croot%3E%3Csentences+id%3D%221%22%3E%3C%2Fsentences%3E%3C%2Froot%3E"
            ),
            r#"<root><sentences id="1"></sentences></root>"#
        );
    }
}
