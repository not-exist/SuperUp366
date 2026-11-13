//! Paper XML parsing for read-aloud tasks.
//!
//! The paper detail (`/client/paper/detail`) returns `paperData`, an XML
//! document whose `<element>` nodes are questions. A read-aloud question is
//! marked `<question_type>9</question_type>` and carries its source sentences
//! in `<attachment>`, which is URL-encoded XML:
//!
//! ```text
//! <sentences id="1"><content><![CDATA[First sentence.]]></content></sentences>
//! ```
//!
//! The recording unit is the **question**, not the sentence: one mp3 per
//! `question_type = 9` element, no matter how many sentences it contains.

use percent_encoding::percent_decode_str;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use crate::error::{ProtocolError, Result};

/// One read-aloud question.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperQuestion {
    pub question_id: String,
    /// Position among read-aloud questions, starting at 1.
    pub number: usize,
    pub score: f64,
    /// Reference duration in seconds, reported to the score endpoint.
    pub duration: u32,
    pub sentences: Vec<String>,
    /// Reference recording, when `pathPrefix` made it absolute.
    pub audio_url: Option<String>,
}

/// All read-aloud questions in one paper.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadAloudPaper {
    pub questions: Vec<PaperQuestion>,
    /// Questions of any other type. Their submit shape is unknown, so callers
    /// must refuse a mixed paper rather than guess.
    pub non_read_aloud: usize,
    pub sentence_count: usize,
    pub total_score: f64,
}

impl ReadAloudPaper {
    /// A mixed paper cannot be submitted through this path.
    pub fn require_all_read_aloud(&self) -> Result<()> {
        if self.questions.is_empty() {
            return Err(ProtocolError::Invalid(
                "paper has no read-aloud questions (question_type = 9)".to_owned(),
            ));
        }
        if self.non_read_aloud > 0 {
            return Err(ProtocolError::Unsupported(format!(
                "paper mixes read-aloud questions with {} other question(s); \
                 their submit shape is not confirmed",
                self.non_read_aloud
            )));
        }
        Ok(())
    }
}

/// Parse `paperData` into read-aloud questions.
///
/// `path_prefix` comes from the same paper-detail payload and is only used to
/// turn a reference audio path into an absolute URL.
pub fn parse_paper(xml: &str, path_prefix: &str) -> Result<ReadAloudPaper> {
    let parsed = questions_from_paper(xml);
    let mut questions = Vec::new();
    let mut non_read_aloud = 0;
    for partial in parsed {
        let question = partial.into_question();
        if question.question_type.trim() != "9" {
            non_read_aloud += 1;
            continue;
        }
        let mut question = PaperQuestion {
            question_id: question.question_id,
            number: questions.len() + 1,
            score: question.score,
            duration: question.duration,
            sentences: question.sentences,
            audio_url: question.audio_url,
        };
        if let Some(url) = question.audio_url.take() {
            question.audio_url = absolute_url(path_prefix, &url);
        }
        questions.push(question);
    }

    let sentence_count = questions.iter().map(|q| q.sentences.len()).sum();
    let total_score = questions.iter().map(|q| q.score).sum();
    Ok(ReadAloudPaper {
        questions,
        non_read_aloud,
        sentence_count,
        total_score,
    })
}

#[derive(Debug, Default)]
struct PartialQuestion {
    question_id: String,
    question_type: String,
    score: f64,
    duration: u32,
    attachment: String,
    question_text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Collecting {
    Nothing,
    Type,
    Score,
    Duration,
    Attachment,
    Text,
}

fn questions_from_paper(xml: &str) -> Vec<PartialQuestion> {
    let mut questions = Vec::new();
    if xml.trim().is_empty() {
        return questions;
    }

    let mut reader = Reader::from_str(xml);
    let mut current: Option<PartialQuestion> = None;
    let mut depth = 0usize;
    let mut collecting = Collecting::Nothing;
    let mut buffer = String::new();

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(event) => event,
        };
        match event {
            Event::Start(start) => {
                if local_name(&start) == "element" {
                    if current.is_none() {
                        current = Some(PartialQuestion {
                            question_id: attribute(&start, "id").unwrap_or_default(),
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
    questions
}

impl PartialQuestion {
    fn into_question(self) -> QuestionFields {
        let mut sentences = sentences_from_attachment(&self.attachment);
        if sentences.is_empty() {
            let text = plain_text(&self.question_text);
            if !text.is_empty() {
                sentences.push(text);
            }
        }
        QuestionFields {
            question_id: self.question_id,
            question_type: self.question_type,
            score: self.score,
            duration: self.duration,
            sentences,
            audio_url: attribute_from_text(&self.question_text, "url"),
        }
    }
}

struct QuestionFields {
    question_id: String,
    question_type: String,
    score: f64,
    duration: u32,
    sentences: Vec<String>,
    audio_url: Option<String>,
}

/// Sentences inside the decoded `<attachment>` document.
fn sentences_from_attachment(xml: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    if xml.trim().is_empty() {
        return sentences;
    }

    let mut reader = Reader::from_str(xml);
    let mut in_sentence = false;
    let mut in_content = false;
    let mut buffer = String::new();

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(event) => event,
        };
        match event {
            Event::Start(start) => match local_name(&start) {
                "sentences" => {
                    in_sentence = true;
                    buffer.clear();
                }
                "content" if in_sentence => in_content = true,
                _ => {}
            },
            Event::CData(text) => {
                if in_content {
                    buffer.push_str(&text);
                }
            }
            Event::Text(text) => {
                if in_content {
                    buffer.push_str(&text);
                }
            }
            Event::End(end) => match end.name().local_name().into_inner() {
                "content" if in_content => in_content = false,
                "sentences" if in_sentence => {
                    in_sentence = false;
                    let sentence = buffer.trim();
                    if !sentence.is_empty() {
                        sentences.push(sentence.to_owned());
                    }
                    buffer.clear();
                }
                _ => {}
            },
            _ => {}
        }
    }
    sentences
}

/// `<attachment>` is URL-encoded; `+` means a space, as in a query string.
fn decode_attachment(encoded: &str) -> String {
    let with_spaces = encoded.replace('+', " ");
    percent_decode_str(&with_spaces)
        .decode_utf8_lossy()
        .into_owned()
}

fn absolute_url(prefix: &str, path: &str) -> Option<String> {
    if path.starts_with("http://") || path.starts_with("https://") {
        return Some(path.to_owned());
    }
    if !prefix.starts_with("http://") && !prefix.starts_with("https://") {
        return None;
    }
    if path.starts_with('/') {
        Some(format!("{}{path}", prefix.trim_end_matches('/')))
    } else {
        Some(format!("{}/{}", prefix.trim_end_matches('/'), path))
    }
}

/// Read one attribute from an HTML/XML fragment.
fn attribute_from_text(text: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = text.find(&needle)? + needle.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn local_name<'a>(start: &'a BytesStart<'_>) -> &'a str {
    start.name().local_name().into_inner()
}

fn attribute(start: &BytesStart<'_>, name: &str) -> Option<String> {
    start
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.local_name().into_inner() == name)
        .and_then(|attribute| attribute.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|value| value.into_owned())
}

fn plain_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    let decoded = text
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAPER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<elements>
  <element id='Q1' type='3'>
    <question_no>1</question_no>
    <question_text><![CDATA[<img url="/Q1/media/a.mp3">Hello world. How are you?]]></question_text>
    <question_score>10.0</question_score>
    <question_type>9</question_type>
    <duration>82</duration>
    <attachment><![CDATA[%3Croot%3E%3Cparagraph%3E%3Csentences+id%3D%221%22%3E%3Ccontent%3E%3C%21%5BCDATA%5BHello+world.+%5D%5D%3E%3C%2Fcontent%3E%3C%2Fsentences%3E%3Csentences+id%3D%222%22%3E%3Ccontent%3E%3C%21%5BCDATA%5BHow+are+you%3F%5D%5D%3E%3C%2Fcontent%3E%3C%2Fsentences%3E%3C%2Fparagraph%3E%3C%2Froot%3E]]></attachment>
  </element>
  <element id='Q2' type='3'>
    <question_score>10.0</question_score>
    <question_type>9</question_type>
    <duration>90</duration>
    <attachment><![CDATA[%3Croot%3E%3Cparagraph%3E%3Csentences+id%3D%221%22%3E%3Ccontent%3E%3C%21%5BCDATA%5BSecond+paragraph.+%5D%5D%3E%3C%2Fcontent%3E%3C%2Fsentences%3E%3C%2Fparagraph%3E%3C%2Froot%3E]]></attachment>
  </element>
</elements>"#;

    #[test]
    fn detects_questions_and_sentences() {
        let paper = parse_paper(PAPER, "https://cdn.example/storage/").unwrap();
        assert_eq!(paper.questions.len(), 2);
        assert_eq!(paper.sentence_count, 3);
        assert_eq!(paper.total_score, 20.0);
        assert_eq!(paper.non_read_aloud, 0);
        assert_eq!(paper.questions[0].question_id, "Q1");
        assert_eq!(paper.questions[0].number, 1);
        assert_eq!(paper.questions[0].duration, 82);
        assert_eq!(
            paper.questions[0].sentences,
            vec!["Hello world.", "How are you?"]
        );
        assert_eq!(
            paper.questions[0].audio_url.as_deref(),
            Some("https://cdn.example/storage/Q1/media/a.mp3")
        );
        assert_eq!(paper.questions[1].number, 2);
    }

    #[test]
    fn rejects_a_mixed_paper() {
        let mixed = r#"<elements>
            <element id='Q1' type='3'><question_type>9</question_type><question_score>10</question_score></element>
            <element id='Q2' type='1'><question_type>1</question_type><question_score>10</question_score></element>
        </elements>"#;
        let paper = parse_paper(mixed, "").unwrap();
        assert_eq!(paper.non_read_aloud, 1);
        assert!(matches!(
            paper.require_all_read_aloud(),
            Err(ProtocolError::Unsupported(_))
        ));
    }

    #[test]
    fn an_empty_paper_has_no_slots() {
        let paper = parse_paper("<elements/>", "").unwrap();
        assert!(paper.questions.is_empty());
        assert!(paper.require_all_read_aloud().is_err());
    }

    #[test]
    fn a_question_without_attachment_falls_back_to_its_text() {
        let xml = r#"<elements><element id='Q1' type='3'>
            <question_text><![CDATA[<img src="x">One sentence only.]]></question_text>
            <question_type>9</question_type><question_score>10</question_score>
        </element></elements>"#;
        let paper = parse_paper(xml, "").unwrap();
        assert_eq!(paper.questions[0].sentences, vec!["One sentence only."]);
    }
}
