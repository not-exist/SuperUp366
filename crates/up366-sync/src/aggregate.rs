//! Cross-class job aggregation.
//!
//! Requirement: the job list is **one flat list across all classes**, in the
//! official order, with each row labelled by its class. It must not be grouped
//! by class, and a job that appears in two classes stays as two rows.
//!
//! Ordering rule: the upstream arrays are authoritative. We keep
//! `(course_order, job_order)` as returned and never re-sort by name, date or
//! status. Deduplication is keyed on `(course_id, job_id)` only.

use up366_core::{CourseId, JobId, JobKind, JobState};

/// One job as returned for one course.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrapedJob {
    pub job_id: JobId,
    /// Position within the upstream `allTaskList` array.
    pub job_order: u32,
    pub name: String,
    pub kind: JobKind,
    pub state: JobState,
}

/// All jobs for one course, with the course's own position in the course list.
#[derive(Debug, Clone, PartialEq)]
pub struct CourseJobs {
    pub course_id: CourseId,
    pub course_name: String,
    /// Position within the upstream course list.
    pub course_order: u32,
    pub jobs: Vec<ScrapedJob>,
}

/// One row of the merged list.
#[derive(Debug, Clone, PartialEq)]
pub struct JobRow {
    pub course_id: CourseId,
    pub course_name: String,
    pub job_id: JobId,
    pub name: String,
    pub kind: JobKind,
    pub state: JobState,
}

/// Merge per-course job lists into one flat, stably ordered list.
///
/// Courses are ordered by `course_order`; within a course the upstream array
/// order is preserved. Duplicate `(course_id, job_id)` pairs keep the first
/// occurrence. The same `job_id` under different courses yields two rows.
pub fn merge_jobs(courses: impl IntoIterator<Item = CourseJobs>) -> Vec<JobRow> {
    let mut ordered: Vec<CourseJobs> = courses.into_iter().collect();
    // Stable sort: courses with equal order keep their input position.
    ordered.sort_by_key(|course| course.course_order);

    let mut rows = Vec::new();
    let mut seen: std::collections::HashSet<(CourseId, JobId)> = std::collections::HashSet::new();

    for course in ordered {
        let mut jobs = course.jobs;
        jobs.sort_by_key(|job| job.job_order);

        for job in jobs {
            let key = (course.course_id.clone(), job.job_id.clone());
            if !seen.insert(key) {
                continue;
            }
            rows.push(JobRow {
                course_id: course.course_id.clone(),
                course_name: course.course_name.clone(),
                job_id: job.job_id,
                name: job.name,
                kind: job.kind,
                state: job.state,
            });
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, order: u32, name: &str) -> ScrapedJob {
        ScrapedJob {
            job_id: JobId::from(id),
            job_order: order,
            name: name.to_owned(),
            kind: JobKind::Unknown,
            state: JobState::Pending,
        }
    }

    fn course(id: &str, name: &str, order: u32, jobs: Vec<ScrapedJob>) -> CourseJobs {
        CourseJobs {
            course_id: CourseId::from(id),
            course_name: name.to_owned(),
            course_order: order,
            jobs,
        }
    }

    #[test]
    fn merges_across_classes_without_grouping() {
        let rows = merge_jobs(vec![
            course("2", "二班", 1, vec![job("b1", 0, "B1")]),
            course("1", "一班", 0, vec![job("a1", 0, "A1"), job("a2", 1, "A2")]),
        ]);

        let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["A1", "A2", "B1"]);
        // Each row still carries its own class label.
        assert_eq!(rows[0].course_name, "一班");
        assert_eq!(rows[2].course_name, "二班");
    }

    #[test]
    fn preserves_upstream_order_within_a_course() {
        let rows = merge_jobs(vec![course(
            "1",
            "一班",
            0,
            vec![job("z", 0, "Z 作业"), job("a", 1, "A 作业")],
        )]);
        // Not alphabetical: array order wins.
        assert_eq!(rows[0].name, "Z 作业");
        assert_eq!(rows[1].name, "A 作业");
    }

    #[test]
    fn same_job_in_two_courses_stays_two_rows() {
        let rows = merge_jobs(vec![
            course("1", "一班", 0, vec![job("j", 0, "同名")]),
            course("2", "二班", 1, vec![job("j", 0, "同名")]),
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].course_id, CourseId::from("1"));
        assert_eq!(rows[1].course_id, CourseId::from("2"));
    }

    #[test]
    fn duplicate_job_within_one_course_is_dropped() {
        let rows = merge_jobs(vec![course(
            "1",
            "一班",
            0,
            vec![job("j", 0, "第一次"), job("j", 1, "重复")],
        )]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "第一次");
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(merge_jobs(Vec::new()).is_empty());
    }
}
