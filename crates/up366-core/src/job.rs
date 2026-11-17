//! Job classification.
//!
//! A "job" (homework / word-picking task) is one item the student must answer.
//! The common kinds are:
//!
//! * [`JobKind::Ordinary`]       – a normal paper.
//! * [`JobKind::WordPk`]        – a word-picking card created from the WordPkx app.
//! * [`JobKind::AppliedWriting`] – an applied-writing (应用文) paper.
//!
//! Applied writing is confirmed by the paper (`question_type = 14`), so the
//! title hint only decides whether the UI offers the writing page; the paper
//! detail is still checked before anything is submitted.
//!
//! Read-aloud duties are not a *kind*: they are ordinary paper jobs that also
//! carry a speaking sub-task. Whether a job contains read-aloud questions is
//! only knowable after the paper detail (paper XML) is parsed, so it is
//! modelled in [`crate::status`] / the sync layer, not here.
//!
//! Classification from the job-list name is a *hint only*. Anything that is
//! not confidently a word-picking card stays [`JobKind::Ordinary`]; the
//! authoritative kind is later reconciled against the paper detail.

use serde::{Deserialize, Serialize};

/// The task nature of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    /// A normal paper (may or may not contain read-aloud questions).
    Ordinary,
    /// A word-picking card synced from the WordPkx app.
    WordPk,
    /// An applied-writing (应用文) paper, confirmed from the paper detail.
    AppliedWriting,
    /// Not yet classified (no reliable signal).
    Unknown,
}

impl JobKind {
    /// Conservative classification from a job title.
    ///
    /// The upstream word-picking tasks are titled with a fixed prefix. We only
    /// commit to [`JobKind::WordPk`] when that prefix is present. Applied
    /// writing uses the course's own title convention (应用文/应用写作).
    /// Everything else is [`JobKind::Unknown`] and must be confirmed against
    /// the paper detail before any kind-specific behaviour runs.
    pub fn hint_from_title(title: &str) -> Self {
        let t = title.trim_start();
        // Upstream WordPkx cards are titled like "单词...", "选词...".
        if t.starts_with("单词") || t.contains("单词卡") {
            JobKind::WordPk
        } else if t.contains("应用文") || t.contains("应用写作") {
            JobKind::AppliedWriting
        } else {
            JobKind::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_pk_hint() {
        assert_eq!(JobKind::hint_from_title("单词卡 - Unit 1"), JobKind::WordPk);
        assert_eq!(JobKind::hint_from_title("选择单词"), JobKind::Unknown);
        assert_eq!(
            JobKind::hint_from_title("Unit 3 阅读理解"),
            JobKind::Unknown
        );
    }

    #[test]
    fn applied_writing_hint_is_conservative() {
        assert_eq!(
            JobKind::hint_from_title("应用文 2026-10-16"),
            JobKind::AppliedWriting
        );
        assert_eq!(
            JobKind::hint_from_title("4月20日 应用写作训练"),
            JobKind::AppliedWriting
        );
        assert_eq!(
            JobKind::hint_from_title("写作训练：My Weekend"),
            JobKind::Unknown
        );
    }
}
