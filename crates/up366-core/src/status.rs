//! Status resolution.
//!
//! A job's `状态` column in the TUI is derived from three sources that the old
//! implementation had to reconcile by hand: the finished-jobs list, the
//! task payload (`taskStatus` / `stuStatus` / `endTime`) and the paper detail
//! (`detailStatus`). Getting this priority order right is the single most
//! error-prone part of the original tool, so it lives here as one pure,
//! heavily-tested function.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The coarse state shown in the job list.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobState {
    /// The student has submitted / finished.
    Finished {
        /// Score if the task is graded; `None` for ungraded tasks.
        score: Option<f64>,
    },
    /// Submitted and returned for revision.
    Returned,
    /// Past the deadline with nothing submitted.
    Expired,
    /// Not yet finished and still within the deadline.
    Pending,
    /// No reliable signal at all.
    Unknown,
}

/// The fine-grained state from the paper-detail endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailState {
    /// `detailStatus` is an unfinished marker.
    Unfinished,
    /// `detailStatus` indicates completion.
    Finished,
    /// No detail information.
    Unknown,
}

/// All raw inputs that feed [`resolve_status`].
///
/// Every field is optional on purpose: any one upstream source may be missing
/// or lag behind, and the resolver must stay total (never panic, never guess
/// worse than `Unknown`).
#[derive(Debug, Clone, Default)]
pub struct StatusInputs {
    /// Score from the finished-jobs list, if the job appears there.
    pub finished_score: Option<f64>,
    /// Whether the finished-jobs list contains this job at all.
    pub in_finished_list: bool,
    /// `taskStatus` from the task payload.
    pub task_status: Option<i64>,
    /// `stuStatus` from the task payload.
    pub stu_status: Option<i64>,
    /// `endTime` (deadline) from the task payload.
    pub end_time: Option<DateTime<Utc>>,
    /// Fine detail state from the paper-detail endpoint.
    pub detail_state: Option<DetailState>,
    /// "Now" for the expiry comparison — injected so tests are deterministic.
    pub now: DateTime<Utc>,
}

/// Resolve the coarse [`JobState`] from mixed signals.
///
/// Priority (matches the H5 UI the old tool imitated):
///
/// 1. **Finished** — present in the finished list, or `taskStatus == 1`, or
///    `stuStatus > 1`. Score is taken from the finished list when available.
/// 2. **Returned** — `stuStatus == 1` and `taskStatus != 1` (assigned back).
/// 3. **Expired** — deadline passed and nothing submitted (detail unfinished or
///    absent).
/// 4. **Pending** — still within the deadline.
/// 5. **Unknown** — nothing usable.
pub fn resolve_status(inputs: &StatusInputs) -> JobState {
    // 1. Finished wins over everything.
    let finished = inputs.in_finished_list
        || inputs.task_status == Some(1)
        || inputs.stu_status.map(|s| s > 1).unwrap_or(false);
    if finished {
        return JobState::Finished {
            score: inputs.finished_score,
        };
    }

    // 2. Returned: stuStatus == 1 while taskStatus is not 1.
    if inputs.stu_status == Some(1) {
        return JobState::Returned;
    }

    // 3. Expired: deadline in the past and not finished.
    if let Some(end) = inputs.end_time {
        if end < inputs.now {
            return JobState::Expired;
        }
    }

    // 4. Pending: we have *some* signal that the job is live.
    let has_signal = inputs.task_status.is_some()
        || inputs.stu_status.is_some()
        || inputs.end_time.is_some()
        || inputs.detail_state.is_some();
    if has_signal {
        return JobState::Pending;
    }

    // 5. Nothing to go on.
    JobState::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2025, 1, 10, 12, 0, 0).unwrap()
    }

    #[test]
    fn finished_from_list_with_score() {
        let i = StatusInputs {
            in_finished_list: true,
            finished_score: Some(93.5),
            now: now(),
            ..Default::default()
        };
        assert_eq!(resolve_status(&i), JobState::Finished { score: Some(93.5) });
    }

    #[test]
    fn finished_from_task_status_beats_deadline() {
        let past = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let i = StatusInputs {
            task_status: Some(1),
            end_time: Some(past),
            now: now(),
            ..Default::default()
        };
        assert_eq!(resolve_status(&i), JobState::Finished { score: None });
    }

    #[test]
    fn returned_wins_over_pending() {
        let future = Utc.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap();
        let i = StatusInputs {
            stu_status: Some(1),
            end_time: Some(future),
            now: now(),
            ..Default::default()
        };
        assert_eq!(resolve_status(&i), JobState::Returned);
    }

    #[test]
    fn expired_when_deadline_passed_and_not_finished() {
        let past = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let i = StatusInputs {
            task_status: Some(0),
            end_time: Some(past),
            detail_state: Some(DetailState::Unfinished),
            now: now(),
            ..Default::default()
        };
        assert_eq!(resolve_status(&i), JobState::Expired);
    }

    #[test]
    fn pending_within_deadline() {
        let future = Utc.with_ymd_and_hms(2025, 2, 1, 0, 0, 0).unwrap();
        let i = StatusInputs {
            task_status: Some(0),
            end_time: Some(future),
            now: now(),
            ..Default::default()
        };
        assert_eq!(resolve_status(&i), JobState::Pending);
    }

    #[test]
    fn unknown_when_no_signal() {
        let i = StatusInputs {
            now: now(),
            ..Default::default()
        };
        assert_eq!(resolve_status(&i), JobState::Unknown);
    }
}
