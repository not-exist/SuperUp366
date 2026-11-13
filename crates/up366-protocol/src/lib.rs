//! Protocol layer: endpoint table, wire encoding, signatures, transport and
//! strongly typed read adapters.

pub mod adapters;
pub mod auth;
pub mod client;
pub mod codec;
pub mod dto;
pub mod endpoint;
pub mod error;
pub mod form;
pub mod session;
pub mod signature;

pub use adapters::{
    answer_detail, answer_detail_v1, answer_detail_v2, book_info, book_plan, book_task_nodes,
    book_task_questions, book_task_words, books, bucket_words, course_finished_jobs, courses,
    finished_tasks, job_info, jobs, paper_detail,
};
pub use auth::{login, login_with_salt, PasswordSalt};
pub use client::{Client, ClientBuilder, CookieJar, DEFAULT_TIMEOUT, USER_AGENT_VALUE};
pub use codec::{parse_envelope, parse_envelope_with_codes, require_data};
pub use dto::{
    AnswerDetail, AnswerDetailParams, Book, BookPlanEntry, BookQuestionRow, BookTaskNode,
    BookWordRow, Course, FinishedJob, FinishedJobs, FinishedTask, Job, JobInfo, Page, Pager,
    PaperDetail, Word, WORD_AUDIO_BASE,
};
pub use endpoint::{get, BodyKind, Endpoint, ENDPOINTS};
pub use error::{ProtocolError, Result};
pub use form::{encode_form, normalize_key};
pub use session::Session;
pub use signature::{
    login_password_hash, login_ut, submit_task_ut, submit_task_ut_flat, PASSWORD_SALT_AT,
    PASSWORD_SALT_COLON,
};
