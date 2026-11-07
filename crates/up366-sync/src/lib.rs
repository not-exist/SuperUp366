//! Orchestration: aggregation, caching policy and submission workflows.
//!
//! Currently only the aggregation rule; the run loop and cache land in later
//! steps.

pub mod aggregate;

pub use aggregate::{merge_jobs, CourseJobs, JobRow, ScrapedJob};
