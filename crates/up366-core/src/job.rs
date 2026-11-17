//! Job classification.
//!
//! A "job" (homework / word-picking task) is one item the student must answer.
//! The common kinds are:
//!
//! * [`JobKind::Ordinary`]       – a normal paper.
//! * [`JobKind::WordPk`]        – a word-picking card created from the WordPkx app.
//! * [`JobKind::AppliedWriting`] – an applied-writing (应用文) paper.
//! * [`JobKind::Dictation`]     – a dictation / memorisation homework candidate.
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
//! Classification from the job-list name is a *hint only*. A dictation hint
//! does not establish the paper format or authorize submission; any
//! kind-specific write behaviour must be confirmed against the paper and its
//! protocol first.

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
    /// A title or job-parameter hint for dictation homework; not submission authorization.
    Dictation,
    /// Not yet classified (no reliable signal).
    Unknown,
}

impl JobKind {
    /// Conservative classification from a job title.
    ///
    /// 默写/听写/拼写 only marks a candidate: its paper and submission protocol
    /// are not established by the title. Everything else is [`JobKind::Unknown`].
    pub fn hint_from_title(title: &str) -> Self {
        let t = title.trim_start();
        // Upstream WordPkx cards are titled like "单词...", "选词...".
        if ["默写", "听写", "拼写"]
            .iter()
            .any(|needle| t.contains(needle))
        {
            JobKind::Dictation
        } else if t.starts_with("单词") || t.contains("单词卡") {
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

    #[test]
    fn dictation_titles_are_candidates_only() {
        for title in ["课文默写练习", "周三听写", "单词拼写训练"] {
            assert_eq!(JobKind::hint_from_title(title), JobKind::Dictation);
        }
        assert_eq!(
            JobKind::hint_from_title("应用文范文默写"),
            JobKind::Dictation
        );
        assert_eq!(JobKind::hint_from_title("英语作业"), JobKind::Unknown);
    }
}
