//! Cache for upstream course / job data.
//!
//! Every row carries `fetched_at`. Reads take an optional maximum age so the
//! sync layer can decide whether the cache is fresh enough or a network fetch
//! is due. Timestamps are Unix epoch milliseconds.

use std::time::Duration;

use sqlx::{Row, SqlitePool};
use up366_core::{AccountId, CourseId, JobId};

use crate::db::now_millis;
use crate::Result;

/// One cached course, in upstream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseRecord {
    pub course_id: CourseId,
    pub name: String,
    pub order_index: i64,
    pub raw_json: String,
    pub fetched_at: i64,
}

/// One cached job. A job may exist under several courses, so the primary key
/// is `(account_id, course_id, job_id)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    pub course_id: CourseId,
    pub job_id: JobId,
    pub content_id: Option<String>,
    pub book_id: Option<String>,
    pub chapter_id: Option<String>,
    pub paper_id: Option<String>,
    pub name: String,
    pub order_index: i64,
    pub raw_json: String,
    pub fetched_at: i64,
}

/// Latest known status of a job.
#[derive(Debug, Clone, PartialEq)]
pub struct JobStatusRecord {
    pub job_id: JobId,
    pub task_status: Option<i64>,
    pub stu_status: Option<i64>,
    pub submit_time: Option<i64>,
    pub score: Option<f64>,
    pub max_score: Option<f64>,
    pub fetched_at: i64,
}

#[derive(Clone)]
pub struct CacheRepo {
    pool: SqlitePool,
}

impl CacheRepo {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update courses in one transaction.
    pub async fn upsert_courses(
        &self,
        account_id: &AccountId,
        courses: &[CourseRecord],
    ) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        for course in courses {
            sqlx::query(
                "INSERT INTO courses (account_id, course_id, name, order_index, raw_json, fetched_at)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(account_id, course_id) DO UPDATE SET
                     name        = excluded.name,
                     order_index = excluded.order_index,
                     raw_json    = excluded.raw_json,
                     fetched_at  = excluded.fetched_at",
            )
            .bind(account_id.as_str())
            .bind(course.course_id.as_str())
            .bind(&course.name)
            .bind(course.order_index)
            .bind(&course.raw_json)
            .bind(course.fetched_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(courses.len())
    }

    /// Cached courses, in upstream order.
    ///
    /// When `max_age` is set, rows older than that are omitted.
    pub async fn courses(
        &self,
        account_id: &AccountId,
        max_age: Option<Duration>,
    ) -> Result<Vec<CourseRecord>> {
        let cutoff = cutoff(max_age);
        let mut sql = String::from(
            "SELECT course_id, name, order_index, raw_json, fetched_at
             FROM courses WHERE account_id = ?",
        );
        if cutoff.is_some() {
            sql.push_str(" AND fetched_at >= ?");
        }
        sql.push_str(" ORDER BY order_index");

        let mut query = sqlx::query(&sql).bind(account_id.as_str());
        if let Some(cutoff) = cutoff {
            query = query.bind(cutoff);
        }

        let rows = query.fetch_all(&self.pool).await?;
        rows.iter().map(map_course).collect()
    }

    /// Insert or update jobs in one transaction.
    pub async fn upsert_jobs(&self, account_id: &AccountId, jobs: &[JobRecord]) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        for job in jobs {
            sqlx::query(
                "INSERT INTO jobs (
                     account_id, course_id, job_id, content_id, book_id, chapter_id,
                     paper_id, name, order_index, raw_json, fetched_at
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(account_id, course_id, job_id) DO UPDATE SET
                     content_id  = excluded.content_id,
                     book_id     = excluded.book_id,
                     chapter_id  = excluded.chapter_id,
                     paper_id    = excluded.paper_id,
                     name        = excluded.name,
                     order_index = excluded.order_index,
                     raw_json    = excluded.raw_json,
                     fetched_at  = excluded.fetched_at",
            )
            .bind(account_id.as_str())
            .bind(job.course_id.as_str())
            .bind(job.job_id.as_str())
            .bind(&job.content_id)
            .bind(&job.book_id)
            .bind(&job.chapter_id)
            .bind(&job.paper_id)
            .bind(&job.name)
            .bind(job.order_index)
            .bind(&job.raw_json)
            .bind(job.fetched_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(jobs.len())
    }

    /// Cached jobs across all courses, ordered by course then upstream order.
    pub async fn jobs(
        &self,
        account_id: &AccountId,
        max_age: Option<Duration>,
    ) -> Result<Vec<JobRecord>> {
        self.query_jobs(account_id, None, max_age).await
    }

    /// Cached jobs for one course.
    pub async fn jobs_for_course(
        &self,
        account_id: &AccountId,
        course_id: &CourseId,
        max_age: Option<Duration>,
    ) -> Result<Vec<JobRecord>> {
        self.query_jobs(account_id, Some(course_id), max_age).await
    }

    async fn query_jobs(
        &self,
        account_id: &AccountId,
        course_id: Option<&CourseId>,
        max_age: Option<Duration>,
    ) -> Result<Vec<JobRecord>> {
        let cutoff = cutoff(max_age);
        let mut sql = String::from(
            "SELECT course_id, job_id, content_id, book_id, chapter_id, paper_id,
                    name, order_index, raw_json, fetched_at
             FROM jobs WHERE account_id = ?",
        );
        if course_id.is_some() {
            sql.push_str(" AND course_id = ?");
        }
        if cutoff.is_some() {
            sql.push_str(" AND fetched_at >= ?");
        }
        sql.push_str(" ORDER BY course_id, order_index");

        let mut query = sqlx::query(&sql).bind(account_id.as_str());
        if let Some(course_id) = course_id {
            query = query.bind(course_id.as_str());
        }
        if let Some(cutoff) = cutoff {
            query = query.bind(cutoff);
        }

        let rows = query.fetch_all(&self.pool).await?;
        rows.iter().map(map_job).collect()
    }

    /// Insert or update one job's status.
    pub async fn upsert_job_status(
        &self,
        account_id: &AccountId,
        status: &JobStatusRecord,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO job_status (
                 account_id, job_id, task_status, stu_status, submit_time,
                 score, max_score, fetched_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(account_id, job_id) DO UPDATE SET
                 task_status = excluded.task_status,
                 stu_status  = excluded.stu_status,
                 submit_time = excluded.submit_time,
                 score       = excluded.score,
                 max_score   = excluded.max_score,
                 fetched_at  = excluded.fetched_at",
        )
        .bind(account_id.as_str())
        .bind(status.job_id.as_str())
        .bind(status.task_status)
        .bind(status.stu_status)
        .bind(status.submit_time)
        .bind(status.score)
        .bind(status.max_score)
        .bind(status.fetched_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read one job's status.
    pub async fn job_status(
        &self,
        account_id: &AccountId,
        job_id: &JobId,
    ) -> Result<Option<JobStatusRecord>> {
        let row = sqlx::query(
            "SELECT job_id, task_status, stu_status, submit_time,
                    score, max_score, fetched_at
             FROM job_status WHERE account_id = ? AND job_id = ?",
        )
        .bind(account_id.as_str())
        .bind(job_id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        row.as_ref().map(map_job_status).transpose()
    }

    /// Drop all cached courses for an account.
    pub async fn delete_courses(&self, account_id: &AccountId) -> Result<u64> {
        let result = sqlx::query("DELETE FROM courses WHERE account_id = ?")
            .bind(account_id.as_str())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Drop all cached jobs for an account.
    pub async fn delete_jobs(&self, account_id: &AccountId) -> Result<u64> {
        let result = sqlx::query("DELETE FROM jobs WHERE account_id = ?")
            .bind(account_id.as_str())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Drop the cached jobs of one course.
    pub async fn delete_jobs_for_course(
        &self,
        account_id: &AccountId,
        course_id: &CourseId,
    ) -> Result<u64> {
        let result = sqlx::query("DELETE FROM jobs WHERE account_id = ? AND course_id = ?")
            .bind(account_id.as_str())
            .bind(course_id.as_str())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// Convert a maximum age into the oldest acceptable `fetched_at`.
fn cutoff(max_age: Option<Duration>) -> Option<i64> {
    max_age.map(|age| {
        let millis = age.as_millis().min(i64::MAX as u128) as i64;
        now_millis().saturating_sub(millis)
    })
}

fn map_course(row: &sqlx::sqlite::SqliteRow) -> Result<CourseRecord> {
    Ok(CourseRecord {
        course_id: CourseId::from(row.try_get::<String, _>("course_id")?),
        name: row.try_get("name")?,
        order_index: row.try_get("order_index")?,
        raw_json: row.try_get("raw_json")?,
        fetched_at: row.try_get("fetched_at")?,
    })
}

fn map_job(row: &sqlx::sqlite::SqliteRow) -> Result<JobRecord> {
    Ok(JobRecord {
        course_id: CourseId::from(row.try_get::<String, _>("course_id")?),
        job_id: JobId::from(row.try_get::<String, _>("job_id")?),
        content_id: row.try_get("content_id")?,
        book_id: row.try_get("book_id")?,
        chapter_id: row.try_get("chapter_id")?,
        paper_id: row.try_get("paper_id")?,
        name: row.try_get("name")?,
        order_index: row.try_get("order_index")?,
        raw_json: row.try_get("raw_json")?,
        fetched_at: row.try_get("fetched_at")?,
    })
}

fn map_job_status(row: &sqlx::sqlite::SqliteRow) -> Result<JobStatusRecord> {
    Ok(JobStatusRecord {
        job_id: JobId::from(row.try_get::<String, _>("job_id")?),
        task_status: row.try_get("task_status")?,
        stu_status: row.try_get("stu_status")?,
        submit_time: row.try_get("submit_time")?,
        score: row.try_get("score")?,
        max_score: row.try_get("max_score")?,
        fetched_at: row.try_get("fetched_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, NewAccount};

    async fn db() -> Database {
        let db = Database::memory().await.unwrap();
        db.accounts()
            .upsert(&NewAccount::new("acc-1"))
            .await
            .unwrap();
        db
    }

    fn course(id: &str, order: i64, fetched_at: i64) -> CourseRecord {
        CourseRecord {
            course_id: CourseId::from(id),
            name: format!("course-{id}"),
            order_index: order,
            raw_json: "{}".into(),
            fetched_at,
        }
    }

    fn job(course: &str, id: &str, order: i64, fetched_at: i64) -> JobRecord {
        JobRecord {
            course_id: CourseId::from(course),
            job_id: JobId::from(id),
            content_id: Some(format!("content-{id}")),
            book_id: None,
            chapter_id: None,
            paper_id: None,
            name: format!("job-{id}"),
            order_index: order,
            raw_json: "{}".into(),
            fetched_at,
        }
    }

    #[tokio::test]
    async fn course_upsert_replaces_and_orders_by_upstream_order() {
        let db = db().await;
        let repo = db.cache();
        let account = AccountId::from("acc-1");
        let now = now_millis();

        repo.upsert_courses(&account, &[course("c2", 1, now), course("c1", 0, now)])
            .await
            .unwrap();
        let rows = repo.courses(&account, None).await.unwrap();
        assert_eq!(
            rows.iter()
                .map(|c| c.course_id.as_str())
                .collect::<Vec<_>>(),
            vec!["c1", "c2"]
        );

        repo.upsert_courses(&account, &[course("c1", 0, now + 1)])
            .await
            .unwrap();
        let rows = repo.courses(&account, None).await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].fetched_at, now + 1);
    }

    #[tokio::test]
    async fn ttl_read_omits_stale_rows() {
        let db = db().await;
        let repo = db.cache();
        let account = AccountId::from("acc-1");
        let now = now_millis();

        repo.upsert_courses(
            &account,
            &[course("fresh", 0, now), course("stale", 1, now - 60_000)],
        )
        .await
        .unwrap();
        repo.upsert_jobs(
            &account,
            &[
                job("c", "fresh", 0, now),
                job("c", "stale", 1, now - 60_000),
            ],
        )
        .await
        .unwrap();

        let all = repo.courses(&account, None).await.unwrap();
        assert_eq!(all.len(), 2);

        let fresh = repo
            .courses(&account, Some(Duration::from_secs(30)))
            .await
            .unwrap();
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].course_id.as_str(), "fresh");

        assert_eq!(
            repo.jobs(&account, Some(Duration::from_secs(30)))
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(repo.jobs(&account, None).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn jobs_are_scoped_by_course_and_deletable() {
        let db = db().await;
        let repo = db.cache();
        let account = AccountId::from("acc-1");
        let now = now_millis();

        repo.upsert_jobs(
            &account,
            &[
                job("c1", "j1", 0, now),
                job("c1", "j2", 1, now),
                job("c2", "j1", 0, now),
            ],
        )
        .await
        .unwrap();

        let c1 = repo
            .jobs_for_course(&account, &CourseId::from("c1"), None)
            .await
            .unwrap();
        assert_eq!(c1.len(), 2);
        assert_eq!(repo.jobs(&account, None).await.unwrap().len(), 3);

        assert_eq!(
            repo.delete_jobs_for_course(&account, &CourseId::from("c1"))
                .await
                .unwrap(),
            2
        );
        assert_eq!(repo.jobs(&account, None).await.unwrap().len(), 1);

        assert_eq!(repo.delete_jobs(&account).await.unwrap(), 1);
        repo.upsert_courses(&account, &[course("c1", 0, now)])
            .await
            .unwrap();
        assert_eq!(repo.delete_courses(&account).await.unwrap(), 1);
        assert!(repo.courses(&account, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn job_status_round_trips() {
        let db = db().await;
        let repo = db.cache();
        let account = AccountId::from("acc-1");
        let id = JobId::from("j1");

        assert!(repo.job_status(&account, &id).await.unwrap().is_none());
        let status = JobStatusRecord {
            job_id: id.clone(),
            task_status: Some(1),
            stu_status: Some(2),
            submit_time: Some(1_700_000_000_000),
            score: Some(93.5),
            max_score: Some(100.0),
            fetched_at: now_millis(),
        };
        repo.upsert_job_status(&account, &status).await.unwrap();
        assert_eq!(repo.job_status(&account, &id).await.unwrap(), Some(status));
    }
}
