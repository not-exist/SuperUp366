//! Applied-writing (应用文) papers: detect, build the answer XML, submit once.
//!
//! The paper marks a writing question with `question_type = 14` and names it
//! `应用文写作` in `qlib_qst_type_name`. The official H5 player serialises one
//! `<element>` per writing question as
//!
//! ```text
//! <element id="Q1"><answers><![CDATA[essay text]]></answers></element>
//! ```
//!
//! and posts the whole document (plus `testId`, `paperId`, `uid` and
//! `duration`) to `xot-api /client/onlinetesting/submit`. The server grades the
//! essay asynchronously; no client-side score is sent.
//!
//! A paper that mixes writing with other question types is refused: this
//! client can only answer the writing part, and submitting the rest empty
//! would silently burn the task.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::paper::{parse_questions, plain_text};
use crate::client::Client;
use crate::error::{ProtocolError, Result};

/// `question_type` of an applied-writing question.
pub const APPLIED_WRITING_QUESTION_TYPE: i64 = 14;

/// The official editor caps input at 3000 characters excluding line breaks.
pub const MAX_WRITING_CHARS: usize = 3000;

/// One writing question detected in a paper.
#[derive(Debug, Clone, PartialEq)]
pub struct WritingQuestion {
    pub question_id: String,
    /// `<question_no>` as the paper spells it.
    pub number: String,
    pub score: f64,
    /// Reference duration in seconds.
    pub duration: u32,
    /// Prompt HTML converted to readable text, paragraphs preserved.
    pub prompt: String,
    /// Suggested word count, when the paper declares one.
    pub word_num: Option<u32>,
}

/// All writing questions in one paper.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedWritingPaper {
    pub questions: Vec<WritingQuestion>,
    /// Questions of any other type. A mixed paper cannot be answered here.
    pub other_questions: usize,
    pub total_score: f64,
}

impl AppliedWritingPaper {
    /// Refuse anything that is not a pure applied-writing paper.
    pub fn require_all_writing(&self) -> Result<()> {
        if self.questions.is_empty() {
            return Err(ProtocolError::Invalid(
                "paper has no applied-writing questions (question_type 14)".to_owned(),
            ));
        }
        if self.other_questions > 0 {
            return Err(ProtocolError::Unsupported(format!(
                "paper mixes applied writing with {} other question(s); \
                 this client can only answer the writing part",
                self.other_questions
            )));
        }
        Ok(())
    }
}

/// One essay the user wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WritingAnswer {
    pub question_id: String,
    pub text: String,
}

/// Parse `paperData` into applied-writing questions.
pub fn parse_applied_writing(xml: &str) -> Result<AppliedWritingPaper> {
    let mut questions = Vec::new();
    let mut other_questions = 0;
    for raw in parse_questions(xml)? {
        let question_type = raw.question_type.trim();
        // `<element type="0/1/2">` captions, text and sections carry no type.
        // `question_type = 99` group headers are layout, not questions. An
        // untyped or otherwise unknown element is treated as a question unless
        // it has an explicit layout type; otherwise it is unsafe to submit.
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
        if question_type != APPLIED_WRITING_QUESTION_TYPE {
            other_questions += 1;
            continue;
        }
        let number = if raw.question_no.is_empty() {
            (questions.len() + 1).to_string()
        } else {
            raw.question_no
        };
        questions.push(WritingQuestion {
            question_id: raw.question_id,
            number,
            score: raw.score,
            duration: raw.duration,
            prompt: plain_text(&raw.question_text),
            word_num: raw.word_num,
        });
    }
    let total_score = questions.iter().map(|question| question.score).sum();
    Ok(AppliedWritingPaper {
        questions,
        other_questions,
        total_score,
    })
}

/// Build the `answerData` XML one submission carries.
pub fn build_answer_data(paper: &AppliedWritingPaper, answers: &[WritingAnswer]) -> Result<String> {
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
        let text = answer.text.trim();
        if text.is_empty() {
            return Err(ProtocolError::Invalid(format!(
                "question {} has an empty answer",
                question.number
            )));
        }
        let length: usize = text
            .chars()
            .filter(|character| *character != '\n')
            .map(char::len_utf16)
            .sum();
        if length > MAX_WRITING_CHARS {
            return Err(ProtocolError::Invalid(format!(
                "question {} has {length} characters; the limit is {MAX_WRITING_CHARS}",
                question.number
            )));
        }
        elements.push_str("<element id=\"");
        elements.push_str(&escape(&question.question_id));
        elements.push_str("\"><answers><![CDATA[");
        elements.push_str(&cdata(text));
        elements.push_str("]]></answers></element>");
    }
    elements.push_str("</elements>");
    Ok(elements)
}

/// Ids a writing submission needs.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedWritingSubmission {
    /// The job's `contentId`, which is the H5 page's `testId`.
    pub test_id: String,
    pub paper_id: String,
    pub uid: i64,
    pub answer_data: String,
    /// Seconds spent on the paper; the server records it as study time.
    pub duration: i64,
    pub begin_time: i64,
    pub end_time: i64,
}

/// Post one writing submission.
///
/// The caller must have shown the answer XML to the user first: this is a
/// write and upstream rejects a second submission of the same job with `-13`.
pub async fn submit_applied_writing(
    client: &Client,
    submission: &AppliedWritingSubmission,
) -> Result<Value> {
    super::onlinetesting::submit_document(
        client,
        &super::onlinetesting::DocumentSubmission {
            test_id: submission.test_id.clone(),
            paper_id: submission.paper_id.clone(),
            uid: submission.uid,
            answer_data: submission.answer_data.clone(),
            duration: submission.duration,
            begin_time: submission.begin_time,
            end_time: submission.end_time,
        },
    )
    .await
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
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;
    use serde_json::json;

    const PAPER: &str = r#"<elements>
        <element id="1" type="0"><element_caption><![CDATA[应用文练习]]></element_caption></element>
        <element id="S1" type="2"><section_text><![CDATA[应用文写作]]></section_text></element>
        <element id="Q1" type="3">
            <question_no>1</question_no>
            <question_text><![CDATA[<p>假定你是李华，写一封建议信。</p><p>注意：80 词左右。</p>]]></question_text>
            <options><wordnum>80</wordnum></options>
            <question_score>15.0</question_score>
            <question_type>14</question_type>
            <duration>1200</duration>
        </element>
    </elements>"#;

    #[test]
    fn detects_a_pure_writing_paper() {
        let paper = parse_applied_writing(PAPER).unwrap();
        assert_eq!(paper.questions.len(), 1);
        assert_eq!(paper.other_questions, 0);
        assert_eq!(paper.total_score, 15.0);
        assert!(paper.require_all_writing().is_ok());

        let question = &paper.questions[0];
        assert_eq!(question.question_id, "Q1");
        assert_eq!(question.number, "1");
        assert_eq!(question.score, 15.0);
        assert_eq!(question.duration, 1200);
        assert_eq!(question.word_num, Some(80));
        assert_eq!(
            question.prompt,
            "假定你是李华，写一封建议信。\n注意：80 词左右。"
        );
    }

    #[test]
    fn a_mixed_paper_is_refused() {
        let mixed = r#"<elements>
            <element id="Q1" type="3"><question_type>4</question_type><question_score>10</question_score></element>
            <element id="Q2" type="3"><question_type>14</question_type><question_score>15</question_score></element>
        </elements>"#;
        let paper = parse_applied_writing(mixed).unwrap();
        assert_eq!(paper.other_questions, 1);
        assert!(matches!(
            paper.require_all_writing(),
            Err(ProtocolError::Unsupported(_))
        ));
    }

    #[test]
    fn a_continuation_writing_paper_is_refused() {
        let paper = parse_applied_writing(
            r#"<elements><element id="Q1" type="3"><question_type>18</question_type><question_score>25</question_score></element></elements>"#,
        )
        .unwrap();
        assert!(paper.questions.is_empty());
        assert_eq!(paper.other_questions, 1);
        assert!(paper.require_all_writing().is_err());
    }

    #[test]
    fn the_answer_document_matches_the_player_serialiser() {
        let paper = parse_applied_writing(PAPER).unwrap();
        let xml = build_answer_data(
            &paper,
            &[WritingAnswer {
                question_id: "Q1".to_owned(),
                text: "Dear Sir,\nI am Li Hua.".to_owned(),
            }],
        )
        .unwrap();
        assert_eq!(
            xml,
            "<elements><element id=\"Q1\"><answers><![CDATA[Dear Sir,\nI am Li Hua.]]></answers></element></elements>"
        );
    }

    #[test]
    fn empty_answers_and_extra_answers_are_rejected() {
        let paper = parse_applied_writing(PAPER).unwrap();
        assert!(build_answer_data(
            &paper,
            &[WritingAnswer {
                question_id: "Q1".to_owned(),
                text: "   ".to_owned()
            }]
        )
        .is_err());
        assert!(build_answer_data(&paper, &[]).is_err());
        assert!(build_answer_data(
            &paper,
            &[
                WritingAnswer {
                    question_id: "Q1".to_owned(),
                    text: "One.".to_owned()
                },
                WritingAnswer {
                    question_id: "Q2".to_owned(),
                    text: "Two.".to_owned()
                }
            ]
        )
        .is_err());
    }

    #[test]
    fn an_untyped_question_is_not_mistaken_for_layout() {
        let paper = parse_applied_writing(
            r#"<elements>
                <element id="Q1" type="3"><question_text><![CDATA[Unknown question type.]]></question_text></element>
                <element id="Q2" type="3"><question_type>14</question_type><question_score>15</question_score></element>
            </elements>"#,
        )
        .unwrap();
        assert_eq!(paper.other_questions, 1);
        assert!(matches!(
            paper.require_all_writing(),
            Err(ProtocolError::Unsupported(_))
        ));
    }

    #[test]
    fn a_self_closing_untyped_question_is_not_mistaken_for_layout() {
        let paper = parse_applied_writing(
            r#"<elements><element id="Q1" type="3"/><element id="Q2" type="3"><question_type>14</question_type><question_score>15</question_score></element></elements>"#,
        )
        .unwrap();
        assert_eq!(paper.other_questions, 1);
        assert!(matches!(
            paper.require_all_writing(),
            Err(ProtocolError::Unsupported(_))
        ));
    }

    #[test]
    fn a_cdata_terminator_in_the_essay_is_split() {
        assert_eq!(cdata("a ]]> b"), "a ]]]]><![CDATA[> b");
    }

    #[tokio::test]
    async fn submit_posts_the_confirmed_form_fields() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/onlinetesting/submit")
                .body_includes("testId=CONTENT-1")
                .body_includes("taskId=CONTENT-1")
                .body_includes("paperId=PAPER-1")
                .body_includes("uid=39370562")
                .body_includes("duration=42")
                .body_includes("beginTime=1700000000000")
                .body_includes("endTime=1700000042000")
                .body_includes("studyDate=")
                .body_includes("uploaded=0")
                .body_includes("answerData");
            then.status(200)
                .json_body(ok(json!({"code": 0, "msg": "success"})));
        });

        let submission = AppliedWritingSubmission {
            test_id: "CONTENT-1".to_owned(),
            paper_id: "PAPER-1".to_owned(),
            uid: 39370562,
            answer_data: "<elements/>".to_owned(),
            duration: 42,
            begin_time: 1_700_000_000_000,
            end_time: 1_700_000_042_000,
        };
        submit_applied_writing(&client(&server), &submission)
            .await
            .unwrap();
        mock.assert_calls(1);
    }
}
