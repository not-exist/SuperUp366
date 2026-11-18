//! Dictation (默写) papers: detect `question_type` 19/20, build the answer XML.
//!
//! The XOT engine keeps a dictation answer in `userData.answerText` and
//! serialises it as
//!
//! ```text
//! <element id="Q1"><answers><![CDATA[text]]></answers></element>
//! ```
//!
//! with no `<pictures>` node. Before the text enters CDATA the engine
//! normalises it: runs of whitespace collapse to one space and typographic
//! single quotes become `'`. An answer that is empty after trimming produces
//! no element at all, so this adapter refuses it instead of silently dropping
//! the question.

use serde::{Deserialize, Serialize};

use super::paper::{parse_questions, plain_text};
use crate::error::{ProtocolError, Result};

/// `question_type` of the two dictation families: 19 (篇章默写), 20 (句子默写).
pub const DICTATION_QUESTION_TYPES: [i64; 2] = [19, 20];

/// One dictation question detected in a paper.
#[derive(Debug, Clone, PartialEq)]
pub struct DictationQuestion {
    pub question_id: String,
    /// `<question_no>` as the paper spells it.
    pub number: String,
    pub score: f64,
    /// Reference duration in seconds.
    pub duration: u32,
    /// Prompt text with its paragraphs preserved.
    pub prompt: String,
}

/// All dictation questions in one paper.
#[derive(Debug, Clone, PartialEq)]
pub struct DictationPaper {
    pub questions: Vec<DictationQuestion>,
    /// Questions of any other type. A mixed paper cannot be answered here.
    pub other_questions: usize,
    pub total_score: f64,
}

impl DictationPaper {
    /// Refuse anything that is not a pure 默写 paper.
    ///
    /// The client can only fill dictation inputs; submitting the rest of a
    /// mixed paper empty would burn the attempt.
    pub fn require_all_dictation(&self) -> Result<()> {
        if self.questions.is_empty() {
            return Err(ProtocolError::Invalid(
                "paper has no dictation questions (question_type 19/20)".to_owned(),
            ));
        }
        if self.other_questions > 0 {
            return Err(ProtocolError::Unsupported(format!(
                "paper mixes dictation with {} other question(s); \
                 this client can only answer the dictation part",
                self.other_questions
            )));
        }
        Ok(())
    }
}

/// One dictation answer supplied by the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DictationAnswer {
    pub question_id: String,
    pub text: String,
}

/// Parse `paperData` into dictation questions.
pub fn parse_dictation(xml: &str) -> Result<DictationPaper> {
    let mut questions = Vec::new();
    let mut other_questions = 0;
    for raw in parse_questions(xml)? {
        let question_type = raw.question_type.trim();
        // `<element type="0/1/2">` captions, text and sections carry no type.
        // `question_type = 99` group headers are layout, not questions.
        if question_type.is_empty() {
            if !matches!(raw.element_type.as_str(), "0" | "1" | "2") {
                other_questions += 1;
            }
            continue;
        }
        let Ok(question_type) = question_type.parse::<i64>() else {
            other_questions += 1;
            continue;
        };
        if question_type == 99 {
            continue;
        }
        if !DICTATION_QUESTION_TYPES.contains(&question_type) {
            other_questions += 1;
            continue;
        }
        let number = if raw.question_no.is_empty() {
            (questions.len() + 1).to_string()
        } else {
            raw.question_no
        };
        questions.push(DictationQuestion {
            question_id: raw.question_id,
            number,
            score: raw.score,
            duration: raw.duration,
            prompt: plain_text(&raw.question_text),
        });
    }
    let total_score = questions.iter().map(|question| question.score).sum();
    Ok(DictationPaper {
        questions,
        other_questions,
        total_score,
    })
}

/// Build the `answerData` XML one submission carries.
///
/// Every question needs a non-empty answer: the engine omits an empty element,
/// which would submit a paper with a silently missing question.
pub fn build_answer_data(paper: &DictationPaper, answers: &[DictationAnswer]) -> Result<String> {
    if answers.len() != paper.questions.len() {
        return Err(ProtocolError::Invalid(format!(
            "expected {} answer(s), got {}",
            paper.questions.len(),
            answers.len()
        )));
    }

    let mut elements = String::from("<elements>");
    for question in &paper.questions {
        let answer = answers
            .iter()
            .find(|answer| answer.question_id == question.question_id)
            .ok_or_else(|| {
                ProtocolError::Invalid(format!("no answer for question {}", question.question_id))
            })?;
        let text = normalise_answer(&answer.text);
        if text.is_empty() {
            return Err(ProtocolError::Invalid(format!(
                "question {} has an empty answer",
                question.number
            )));
        }
        elements.push_str("<element id=\"");
        elements.push_str(&escape(&question.question_id));
        elements.push_str("\"><answers><![CDATA[");
        elements.push_str(&cdata(&text));
        elements.push_str("]]></answers></element>");
    }
    elements.push_str("</elements>");
    Ok(elements)
}

/// Reproduce the XOT engine's pre-submit normalisation.
///
/// Whitespace runs (including newlines) collapse to one space, leading and
/// trailing whitespace disappears, and `‘ ’` become `'`.
pub fn normalise_answer(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        let character = match character {
            '‘' | '’' => '\'',
            other => other,
        };
        if character.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(character);
    }
    out
}

/// Escape a value placed inside a double-quoted XML attribute.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Split a CDATA terminator so user text cannot close the section early.
fn cdata(text: &str) -> String {
    text.replace("]]>", "]]]]><![CDATA[>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProtocolError;

    const PAPER: &str = r#"<elements>
        <element id="1" type="0"><element_caption><![CDATA[默写练习]]></element_caption></element>
        <element id="S1" type="2"><section_text><![CDATA[Unit 5]]></section_text></element>
        <element id="Q19" type="3">
            <question_no>1</question_no>
            <question_text><![CDATA[<p>默写第 4 段。</p>]]></question_text>
            <question_score>10.0</question_score>
            <question_type>19</question_type>
            <duration>300</duration>
        </element>
        <element id="Q20" type="3">
            <question_no>2</question_no>
            <question_text><![CDATA[<p>默写句子。</p>]]></question_text>
            <question_score>5.0</question_score>
            <question_type>20</question_type>
            <duration>120</duration>
        </element>
    </elements>"#;

    #[test]
    fn detects_a_pure_dictation_paper() {
        let paper = parse_dictation(PAPER).unwrap();
        assert_eq!(paper.questions.len(), 2);
        assert_eq!(paper.other_questions, 0);
        assert_eq!(paper.total_score, 15.0);
        assert!(paper.require_all_dictation().is_ok());

        let question = &paper.questions[0];
        assert_eq!(question.question_id, "Q19");
        assert_eq!(question.number, "1");
        assert_eq!(question.score, 10.0);
        assert_eq!(question.duration, 300);
        assert_eq!(question.prompt, "默写第 4 段。");
    }

    #[test]
    fn a_mixed_paper_is_refused() {
        let mixed = r#"<elements>
            <element id="Q1" type="3"><question_type>4</question_type><question_score>2</question_score></element>
            <element id="Q2" type="3"><question_type>19</question_type><question_score>10</question_score></element>
        </elements>"#;
        let paper = parse_dictation(mixed).unwrap();
        assert_eq!(paper.questions.len(), 1);
        assert_eq!(paper.other_questions, 1);
        assert!(matches!(
            paper.require_all_dictation(),
            Err(ProtocolError::Unsupported(_))
        ));
    }

    #[test]
    fn a_paper_without_dictation_is_refused() {
        let paper = parse_dictation(
            r#"<elements><element id="Q1" type="3"><question_type>4</question_type></element></elements>"#,
        )
        .unwrap();
        assert!(paper.questions.is_empty());
        assert!(paper.require_all_dictation().is_err());
    }

    #[test]
    fn the_answer_document_matches_the_engine_serialiser() {
        let paper = parse_dictation(PAPER).unwrap();
        let xml = build_answer_data(
            &paper,
            &[
                DictationAnswer {
                    question_id: "Q19".to_owned(),
                    text: "  First line\nsecond line.  ".to_owned(),
                },
                DictationAnswer {
                    question_id: "Q20".to_owned(),
                    text: "It\u{2019}s mine.".to_owned(),
                },
            ],
        )
        .unwrap();
        assert_eq!(
            xml,
            "<elements>\
             <element id=\"Q19\"><answers><![CDATA[First line second line.]]></answers></element>\
             <element id=\"Q20\"><answers><![CDATA[It's mine.]]></answers></element>\
             </elements>"
        );
    }

    #[test]
    fn empty_answers_and_extra_answers_are_rejected() {
        let paper = parse_dictation(PAPER).unwrap();
        assert!(build_answer_data(
            &paper,
            &[
                DictationAnswer {
                    question_id: "Q19".to_owned(),
                    text: "   ".to_owned()
                },
                DictationAnswer {
                    question_id: "Q20".to_owned(),
                    text: "Two.".to_owned()
                }
            ]
        )
        .is_err());
        assert!(build_answer_data(&paper, &[]).is_err());
        assert!(build_answer_data(
            &paper,
            &[
                DictationAnswer {
                    question_id: "Q19".to_owned(),
                    text: "One.".to_owned()
                },
                DictationAnswer {
                    question_id: "Q20".to_owned(),
                    text: "Two.".to_owned()
                },
                DictationAnswer {
                    question_id: "Q21".to_owned(),
                    text: "Three.".to_owned()
                }
            ]
        )
        .is_err());
    }

    #[test]
    fn normalisation_collapses_whitespace_and_quotes() {
        assert_eq!(normalise_answer("  a\n\tb  "), "a b");
        assert_eq!(normalise_answer("\u{2018}hi\u{2019}"), "'hi'");
        assert_eq!(normalise_answer("   "), "");
    }
}
