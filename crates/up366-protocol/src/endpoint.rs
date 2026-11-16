//! Frozen endpoint table.
//!
//! Every call the client makes goes through one of these entries. URLs are
//! never assembled from caller-supplied strings, which keeps the credential
//! surface limited to hosts we deliberately listed.

use serde::{Deserialize, Serialize};

/// How the request body is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyKind {
    /// `application/x-www-form-urlencoded` POST.
    Form,
    /// `application/json` POST.
    Json,
    /// `application/json` POST with a gzipped body (`Content-Encoding: gzip`).
    JsonGzip,
    /// GET with query parameters.
    Query,
    /// No body.
    None,
}

/// One upstream endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    /// Stable logical name used in logs and errors.
    pub name: &'static str,
    /// Scheme + host, for example `https://course-api.up366.cn`.
    pub base_url: &'static str,
    /// Path beginning with `/`.
    pub path: &'static str,
    /// Whether the call mutates server state.
    pub write: bool,
    /// Request body encoding.
    pub body: BodyKind,
}

impl Endpoint {
    /// Full URL.
    pub fn url(&self) -> String {
        format!("{}{}", self.base_url, self.path)
    }
}

const fn ep(
    name: &'static str,
    base_url: &'static str,
    path: &'static str,
    write: bool,
    body: BodyKind,
) -> Endpoint {
    Endpoint {
        name,
        base_url,
        path,
        write,
        body,
    }
}

const SSO: &str = "https://sso.up366.cn";
const COURSE_API: &str = "https://course-api.up366.cn";
const STUDYTASK_API: &str = "https://studytask-api.up366.cn";
const BOOK_API: &str = "https://book-api.up366.cn";
const DSB_API: &str = "https://dsb-api.up366.cn";
const QLIB_API: &str = "https://qlib-api.up366.cn";
const XOT_API: &str = "https://xot-api.up366.cn";
const WORDS_V2_API: &str = "https://words-v2-api.up366.cn";
const WORDSBTL_API: &str = "https://wordsbtl-api.up366.cn";
const STUDY_API: &str = "https://study-api.up366.cn";
const SETUP_API: &str = "https://setup-api.up366.cn";
const USER_API: &str = "https://user-api.up366.cn";
const ERROR_NOTEBOOK_API: &str = "https://error-notebook-api.up366.cn";

/// Endpoints used by this client.
///
/// Deliberately small: only what requirements 1–9 need. The archive lists
/// dozens more (`recon/api.md`); they are out of scope.
pub const ENDPOINTS: &[Endpoint] = &[
    // --- auth ---------------------------------------------------------
    ep("login", SSO, "/v2/tickets-v3", true, BodyKind::Form),
    ep(
        "service_ticket",
        SSO,
        "/v1/serviceTicket",
        false,
        BodyKind::Form,
    ),
    ep(
        "current_user",
        USER_API,
        "/client/user/currentuser",
        false,
        BodyKind::None,
    ),
    // --- catalog ------------------------------------------------------
    ep(
        "courses",
        COURSE_API,
        "/client/course/list-of-student/v2",
        false,
        BodyKind::Form,
    ),
    ep(
        "jobs",
        STUDYTASK_API,
        "/client/student/course/job",
        false,
        BodyKind::Form,
    ),
    ep(
        "job_info",
        STUDYTASK_API,
        "/client/student/job/info",
        false,
        BodyKind::Form,
    ),
    ep(
        "course_finished_jobs",
        STUDYTASK_API,
        "/client/student/all/course/finish/job/v2",
        false,
        BodyKind::Form,
    ),
    ep(
        "finished_tasks",
        DSB_API,
        "/client/book/finished-task/list",
        false,
        BodyKind::Form,
    ),
    // --- books --------------------------------------------------------
    ep(
        "books",
        BOOK_API,
        "/client/books/buy-book",
        false,
        BodyKind::None,
    ),
    // Books attached to a course are not in `buy-book`: the shelf is the
    // union of the two, so the course list is fetched as well.
    ep(
        "course_books",
        COURSE_API,
        "/client/student/course/list",
        false,
        BodyKind::Form,
    ),
    ep(
        "book_info",
        BOOK_API,
        "/client/v2/book/info",
        false,
        BodyKind::Form,
    ),
    ep(
        "book_plan",
        STUDYTASK_API,
        "/client/book/plan",
        false,
        BodyKind::Form,
    ),
    // The book tree. Course books (`bookType = 2`) only answer their pages
    // when the assigning course is sent; a page's `id` is the assignment id
    // and its `paperId` is what the textbook reader opens.
    ep(
        "book_tree",
        BOOK_API,
        "/client/books/tree",
        false,
        BodyKind::Form,
    ),
    // The official book detail view: chapters and assignments with the
    // student's scores, and the per-task detail behind each row.
    ep(
        "book_task_nodes",
        DSB_API,
        "/client/student/chapter-task-score/list",
        false,
        BodyKind::Form,
    ),
    ep(
        "book_task_questions",
        DSB_API,
        "/client/question/student/task/question-score/list",
        false,
        BodyKind::Form,
    ),
    ep(
        "book_task_words",
        DSB_API,
        "/client/word/task/student/word-skill-get/list",
        false,
        BodyKind::Form,
    ),
    // --- answers / papers --------------------------------------------
    ep(
        "answer_detail_v2",
        DSB_API,
        "/client/student/task/answer/detail/v2",
        false,
        BodyKind::Form,
    ),
    ep(
        "answer_detail_v1",
        DSB_API,
        "/client/student/task/answer/detail",
        false,
        BodyKind::Query,
    ),
    ep(
        "paper_detail",
        QLIB_API,
        "/client/paper/detail",
        false,
        BodyKind::Query,
    ),
    // The course-book reader's paper source. The PC client never calls it for
    // its shelf, but `textbook.html` does, and it is how a course-book page
    // gets its paper without a scheduled job.
    ep(
        "textbook_paperinfo",
        XOT_API,
        "/client/textbook/paperinfo",
        false,
        BodyKind::Form,
    ),
    // --- word pk ------------------------------------------------------
    ep(
        "bucket_words",
        WORDS_V2_API,
        "/client/sync/teaching/bucket/detail-info",
        false,
        BodyKind::Form,
    ),
    // Method is disputed between the archive (GET) and the old client
    // (POST with GET fallback). Kept as POST here; the probe step resolves it.
    ep(
        "word_pk_submit",
        WORDSBTL_API,
        "/client/wordsbtl/student/submit",
        true,
        BodyKind::Form,
    ),
    // --- scoring / media ---------------------------------------------
    ep(
        "task_score_submit",
        STUDY_API,
        "/client/task/score/submit/v2",
        true,
        BodyKind::Form,
    ),
    ep(
        "aliyun_oss_token",
        SETUP_API,
        "/front/aliyunoss/token/v2",
        false,
        BodyKind::Query,
    ),
    ep(
        "current_time",
        SETUP_API,
        "/front/current-time",
        false,
        BodyKind::Form,
    ),
    // --- error-notebook correction -----------------------------------
    ep(
        "pm_overview",
        ERROR_NOTEBOOK_API,
        "/client/personalmistake/overview",
        false,
        BodyKind::Json,
    ),
    ep(
        "pm_correction_draw",
        ERROR_NOTEBOOK_API,
        "/client/personalmistake/correction/draw",
        false,
        BodyKind::Form,
    ),
    ep(
        "pm_correction_status",
        ERROR_NOTEBOOK_API,
        "/client/personalmistake/correction/status",
        false,
        BodyKind::Form,
    ),
    // The correction answer page submits through the Electron main process,
    // which gzips the JSON body. The plain `/submit/v2` is a different branch.
    ep(
        "task_score_gzip_submit",
        STUDY_API,
        "/client/task/score/gzip/submit",
        true,
        BodyKind::JsonGzip,
    ),
    // "错题消灭" follow-up: `jobJson` is a JSON string.
    ep(
        "job_submit",
        STUDY_API,
        "/client/job/submit",
        true,
        BodyKind::Form,
    ),
];

/// Look up an endpoint by its logical name.
pub fn get(name: &str) -> Option<&'static Endpoint> {
    ENDPOINTS.iter().find(|e| e.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn names_are_unique() {
        let mut seen = HashSet::new();
        for e in ENDPOINTS {
            assert!(seen.insert(e.name), "duplicate endpoint name: {}", e.name);
        }
    }

    #[test]
    fn every_path_starts_with_slash_and_host_is_https() {
        for e in ENDPOINTS {
            assert!(e.path.starts_with('/'), "bad path for {}", e.name);
            assert!(
                e.base_url.starts_with("https://"),
                "bad host for {}",
                e.name
            );
            assert!(!e.base_url.ends_with('/'), "trailing slash for {}", e.name);
        }
    }

    #[test]
    fn url_joins_without_double_slash() {
        let e = get("jobs").unwrap();
        assert_eq!(
            e.url(),
            "https://studytask-api.up366.cn/client/student/course/job"
        );
    }

    #[test]
    fn only_expected_endpoints_are_writes() {
        let writes: Vec<_> = ENDPOINTS
            .iter()
            .filter(|e| e.write)
            .map(|e| e.name)
            .collect();
        assert_eq!(
            writes,
            vec![
                "login",
                "word_pk_submit",
                "task_score_submit",
                "task_score_gzip_submit",
                "job_submit"
            ]
        );
    }
}
