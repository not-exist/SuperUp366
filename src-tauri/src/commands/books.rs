//! Book commands: shelf, book detail and the per-task detail view.
//!
//! The detail list comes from `/client/student/chapter-task-score/list`, the
//! endpoint the official reader uses. It returns the book's chapters and
//! assignments as one flat array; each assignment's `cid` is also the id that
//! `/client/book/finished-task/list` keys on, so progress is a set
//! intersection. `taskType = 2` marks a vocabulary assignment; every other
//! row is an ordinary paper. Clicking a row opens its read-only detail, which
//! the upstream endpoints can answer without a job row.
//!
//! A recording assignment is the exception: submitting it requires the job id
//! (`studyTaskId`), which the book endpoints do not return. When the same task
//! also appears in the account's job list, its `contentId` matches the book
//! `cid`, so the detail view resolves and exposes that job id and the UI can
//! open the read-aloud page instead of the read-only view.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use up366_core::JobKind;
use up366_protocol::{
    adapters,
    dto::{Book, BookTaskNode, FinishedTask},
};

use crate::commands::uid_for;
use crate::error::{AppError, CommandResult};
use crate::session::AppState;
use crate::state::StatusView;

/// How many parent links are followed before a chapter chain is treated as
/// broken. Real trees are three levels deep; the bound only guards cycles.
const MAX_CHAPTER_DEPTH: usize = 16;

/// One shelf entry. Deliberately metadata only: computing progress needs the
/// whole assignment list (megabytes for a large book), so it is only done on
/// the detail page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookListItem {
    pub book_id: String,
    pub name: String,
    pub subject: String,
    pub cover_url: Option<String>,
    /// `packType == 9` books use the newer reader.
    pub is_2025_reader: bool,
    /// 1 = purchased, 2 = class-assigned. `book_detail` must send it back: a
    /// type-2 book requested as type 1 has no `data` in `book_info`.
    pub book_type: i64,
    /// The course that assigned this book, when one did. The book detail
    /// endpoint always needs a course, so a purchased row leaves this empty
    /// and the command falls back to the account's first course.
    pub course_id: Option<String>,
}

/// One page of a book, rendered as a job row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookJobView {
    /// The id both the tree and the finished list use: a task's `id` for
    /// ordinary books, a content page's `id` for type-2 course books.
    pub job_id: String,
    pub name: String,
    pub kind: JobKind,
    pub status: StatusView,
    pub score: Option<f64>,
    /// Book pages carry no deadline.
    pub deadline: Option<String>,
    pub read_aloud: bool,
    /// The matching job-list id when this book task is also a scheduled job.
    ///
    /// A book task id is not a job id, and the read-aloud submit row needs
    /// `studyTaskId = jobId`; without this the recording page cannot be opened
    /// from the book view (it would look the book id up in the job cache and
    /// fail). `None` means the task is not in the job list. For a class-assigned
    /// course book the page can still be submitted through the reader's
    /// textbook branch, which `readaloud_book` advertises.
    pub readaloud_job_id: Option<String>,
    /// A recording page of a course book (`bookType = 2`). Its paper comes
    /// from the book tree and the submit row uses an empty `studyTaskId`, so
    /// it is submittable even though it is not a scheduled job.
    pub readaloud_book: bool,
}

/// One top-level chapter and every page below it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChapterView {
    pub chapter_id: String,
    pub title: String,
    pub jobs: Vec<BookJobView>,
}

/// Book detail: metadata, progress, and the top-level chapters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookDetailView {
    pub book_id: String,
    pub name: String,
    pub subject: String,
    pub cover_url: Option<String>,
    pub is_2025_reader: bool,
    /// The course the task list was read in. Purchased books carry no course
    /// on the shelf, so the command picks the account's first one.
    pub course_id: String,
    /// Assignments that appear in the finished-task list.
    pub progress: usize,
    /// Assignments the reader lists.
    pub total: usize,
    /// Not exposed by the book payloads observed so far.
    pub teacher_name: Option<String>,
    pub chapters: Vec<ChapterView>,
}

/// The student's shelf.
#[tauri::command(rename_all = "snake_case")]
pub async fn books_list(
    account_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<Vec<BookListItem>> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let purchased = adapters::books(&client)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let assigned = adapters::course_books(&client)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    Ok(shelf_items(&purchased, &assigned))
}

/// The shelf is the union of purchased goods and course books.
///
/// `buy-book` only lists what the student bought; 新概念英语 and the other
/// class-assigned books are only in `/client/student/course/list`. Order is
/// purchase order first, then course order, and a book in both lists is kept
/// once (the purchased row wins because it carries the cover and validity).
fn shelf_items(purchased: &[Book], assigned: &[(String, Book)]) -> Vec<BookListItem> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    let mut items: Vec<BookListItem> = Vec::new();
    for book in purchased {
        let Some(id) = book.id() else {
            continue;
        };
        if seen.contains_key(id) {
            continue;
        }
        seen.insert(id, items.len());
        items.push(book_list_item(book, None));
    }
    // A purchased row wins the metadata, but the assigned copy still knows the
    // course, which `book_detail` needs for the tree.
    for (course_id, book) in assigned {
        let Some(id) = book.id() else {
            continue;
        };
        match seen.get(id) {
            Some(&position) => {
                if items[position].course_id.is_none() {
                    items[position].course_id = Some(course_id.clone());
                }
            }
            None => {
                seen.insert(id, items.len());
                items.push(book_list_item(book, Some(course_id.clone())));
            }
        }
    }
    items
}

fn book_list_item(book: &Book, course_id: Option<String>) -> BookListItem {
    BookListItem {
        book_id: book.id().unwrap_or_default().to_owned(),
        name: book.title().to_owned(),
        subject: book.subject_id.clone().unwrap_or_default(),
        cover_url: book.cover().map(str::to_owned),
        is_2025_reader: book.is_2025_reader(),
        book_type: book.book_type(),
        course_id,
    }
}

/// One book with its chapters and per-page completion.
#[tauri::command(rename_all = "snake_case")]
pub async fn book_detail(
    account_id: String,
    book_id: String,
    book_type: i64,
    course_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> CommandResult<BookDetailView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let book = adapters::book_info(&client, &book_id, book_type)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let course_id = effective_course(&state, &account_id, &client, course_id).await?;
    let uid = uid_for(&state, &account_id, &client).await?;
    let nodes = adapters::book_task_nodes(&client, &course_id, &book_id, &uid)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let finished = adapters::finished_tasks(&client, &book_id)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    let jobs = job_rows(&state, &account_id, &client).await?;
    let (chapters, progress, total) =
        chapters_from_nodes(&nodes, &finished, &jobs, &book_id, &course_id, book_type);

    Ok(BookDetailView {
        book_id: book.id().unwrap_or_default().to_owned(),
        name: book.title().to_owned(),
        subject: book.subject_id.clone().unwrap_or_default(),
        cover_url: book.cover().map(str::to_owned),
        is_2025_reader: book.is_2025_reader(),
        course_id,
        progress,
        total,
        teacher_name: None,
        chapters,
    })
}

/// Cached job rows, refreshed once when the cache has never been filled.
///
/// A book is often opened before the job list has been fetched; the recording
/// tasks still need their job ids, so one catalog refresh runs in that case.
/// A failed refresh is not fatal here: the book detail is readable on its own
/// and the rows simply stay read-only.
async fn job_rows(
    state: &AppState,
    account_id: &up366_core::AccountId,
    client: &up366_protocol::Client,
) -> CommandResult<Vec<up366_store::JobRecord>> {
    let cached = state
        .db
        .cache()
        .jobs(account_id, None)
        .await
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    if !cached.is_empty() {
        return Ok(cached);
    }
    if crate::commands::catalog::refresh_catalog(state, account_id, client)
        .await
        .is_ok()
    {
        return state
            .db
            .cache()
            .jobs(account_id, None)
            .await
            .map_err(|error| AppError::new("storage_error", error.to_string()));
    }
    Ok(cached)
}

/// The course whose task list is read.
///
/// The shelf sends the assigning course for a class book. A purchased book has
/// none, but the score endpoint requires one and any course the student belongs
/// to answers the same tree, so the account's first course is used.
async fn effective_course(
    state: &AppState,
    account_id: &up366_core::AccountId,
    client: &up366_protocol::Client,
    course_id: Option<String>,
) -> CommandResult<String> {
    if let Some(course_id) = course_id.filter(|id| !id.trim().is_empty()) {
        return Ok(course_id);
    }
    let cached = state
        .db
        .cache()
        .courses(account_id, None)
        .await
        .map_err(|error| AppError::new("storage_error", error.to_string()))?;
    if let Some(course) = cached.first() {
        return Ok(course.course_id.to_string());
    }
    let courses = adapters::courses(client)
        .await
        .map_err(|error| state.session_error(account_id, client, error))?;
    courses
        .first()
        .and_then(|course| course.id())
        .map(str::to_owned)
        .ok_or_else(|| AppError::new("invalid_input", "账号没有班级，无法读取书本作业"))
}

/// One question of a book assignment, as the reader shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookQuestionView {
    pub question_id: String,
    pub number: String,
    pub type_name: String,
    pub knowledge: String,
    /// A recording URL for read-aloud questions, an answer string otherwise.
    pub my_answer: String,
    pub correct_answer: String,
    pub max_score: Option<f64>,
    pub score: Option<f64>,
    pub class_average: Option<f64>,
    pub read_aloud: bool,
}

/// One vocabulary row of a book assignment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookWordView {
    pub word: String,
    pub mastered: bool,
}

/// Read-only detail of one book assignment.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookTaskView {
    pub kind: JobKind,
    /// Whether any question is a recording (`questionTypeNum` 9/12/13).
    pub read_aloud: bool,
    pub questions: Vec<BookQuestionView>,
    pub words: Vec<BookWordView>,
}

/// Open one book assignment: vocabulary rows or its question rows.
///
/// A book task id is not a job id, so this deliberately does not go through
/// the job cache. The upstream detail endpoints key on
/// `(courseId, bookId, taskId, uid)` and work for finished and unfinished
/// assignments alike.
#[tauri::command(rename_all = "snake_case")]
pub async fn book_task_get(
    account_id: String,
    book_id: String,
    course_id: Option<String>,
    task_id: String,
    word_task: bool,
    state: tauri::State<'_, AppState>,
) -> CommandResult<BookTaskView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let course_id = effective_course(&state, &account_id, &client, course_id).await?;
    let uid = uid_for(&state, &account_id, &client).await?;

    if word_task {
        let words = adapters::book_task_words(&client, &course_id, &book_id, &task_id, &uid)
            .await
            .map_err(|error| state.session_error(&account_id, &client, error))?;
        return Ok(BookTaskView {
            kind: JobKind::WordPk,
            read_aloud: false,
            questions: Vec::new(),
            words: words
                .into_iter()
                .map(|row| {
                    let mastered = row.mastered();
                    BookWordView {
                        word: row.word,
                        mastered,
                    }
                })
                .collect(),
        });
    }

    let rows = adapters::book_task_questions(&client, &course_id, &book_id, &task_id, &uid)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let read_aloud = rows.iter().any(|row| row.is_read_aloud());
    Ok(BookTaskView {
        kind: JobKind::Ordinary,
        read_aloud,
        questions: rows
            .into_iter()
            .map(|row| {
                let read_aloud = row.is_read_aloud();
                BookQuestionView {
                    question_id: row.question_id,
                    number: row.question_no,
                    type_name: row.qst_type_name,
                    knowledge: row.knowledge,
                    my_answer: row.my_answer,
                    correct_answer: row.correct_answer,
                    max_score: row.question_score,
                    score: row.score,
                    class_average: row.avg_score,
                    read_aloud,
                }
            })
            .collect(),
        words: Vec::new(),
    })
}

/// Group every assignment under the top-level chapter that owns it.
///
/// The upstream list is flat: chapters (`nodeType = 1`) and assignments
/// (`nodeType = 2`). Order is the upstream array order; nothing is re-sorted.
/// Returns the chapters plus `(progress, total)`; an assignment counts as
/// finished when its id appears in the finished-task list.
fn chapters_from_nodes(
    nodes: &[BookTaskNode],
    finished: &[FinishedTask],
    jobs: &[up366_store::JobRecord],
    book_id: &str,
    course_id: &str,
    book_type: i64,
) -> (Vec<ChapterView>, usize, usize) {
    let parents: HashMap<&str, &str> = nodes
        .iter()
        .filter(|node| node.is_chapter())
        .map(|node| (node.cid.as_str(), node.pid.as_deref().unwrap_or("")))
        .collect();
    let chapter_ids: HashSet<&str> = parents.keys().copied().collect();

    let mut chapters: Vec<ChapterView> = Vec::new();
    let mut top_index: HashMap<&str, usize> = HashMap::new();
    for node in nodes.iter().filter(|node| node.is_chapter()) {
        if node
            .pid
            .as_deref()
            .is_some_and(|parent| chapter_ids.contains(parent))
        {
            continue;
        }
        top_index.insert(node.cid.as_str(), chapters.len());
        chapters.push(ChapterView {
            chapter_id: node.cid.clone(),
            title: node.node_name.clone(),
            jobs: Vec::new(),
        });
    }

    // taskId -> score. An absent id means the assignment is unfinished.
    let scores: HashMap<&str, Option<f64>> = finished
        .iter()
        .filter_map(|task| Some((task.task_id.as_deref()?, task.score)))
        .collect();

    let mut progress = 0;
    let mut total = 0;
    let mut seen: HashSet<&str> = HashSet::new();
    for node in nodes.iter().filter(|node| node.is_task()) {
        if !seen.insert(node.cid.as_str()) {
            continue;
        }
        total += 1;

        let record = scores.get(node.cid.as_str());
        let status = match record {
            Some(score) => {
                progress += 1;
                StatusView::Finished { score: *score }
            }
            None => StatusView::Pending,
        };

        let Some(top) = top_chapter(node.pid.as_deref().unwrap_or(""), &parents, &chapter_ids)
        else {
            continue;
        };
        let Some(&index) = top_index.get(top) else {
            continue;
        };
        chapters[index].jobs.push(BookJobView {
            job_id: node.cid.clone(),
            name: if node.node_name.is_empty() {
                "任务".to_owned()
            } else {
                node.node_name.clone()
            },
            kind: if node.is_word_task() {
                JobKind::WordPk
            } else {
                JobKind::Ordinary
            },
            status,
            score: record.copied().flatten(),
            deadline: None,
            read_aloud: is_read_aloud_name(&node.node_name),
            readaloud_job_id: if is_read_aloud_name(&node.node_name) {
                matching_job_id(node, jobs, book_id, course_id)
            } else {
                None
            },
            readaloud_book: book_type == 2 && is_read_aloud_name(&node.node_name),
        });
    }

    (chapters, progress, total)
}

/// Book tasks name their recordings rather than carrying an open-type marker,
/// so the title is the same kind of hint the job list uses. `复述` is the
/// `question_type = 13` retelling duty; `问答` is deliberately absent because
/// ordinary reading papers use it too.
fn is_read_aloud_name(name: &str) -> bool {
    ["跟读", "朗读", "听读", "背诵", "复述"]
        .iter()
        .any(|needle| name.contains(needle))
}

/// The job whose `contentId` is this book task's `cid`.
///
/// Book and job endpoints describe the same task with different keys, so the
/// content id is the join key. Several classes may carry the same job; prefer
/// the row from this book and this course, then keep the upstream first.
fn matching_job_id(
    node: &BookTaskNode,
    jobs: &[up366_store::JobRecord],
    book_id: &str,
    course_id: &str,
) -> Option<String> {
    let mut best: Option<(u8, &up366_store::JobRecord)> = None;
    for job in jobs {
        if job.content_id.as_deref() != Some(node.cid.as_str()) {
            continue;
        }
        let score = u8::from(job.book_id.as_deref() == Some(book_id)) * 2
            + u8::from(job.course_id.as_str() == course_id);
        if best.is_none_or(|(current, _)| score > current) {
            best = Some((score, job));
        }
    }
    best.map(|(_, job)| job.job_id.as_str().to_owned())
}

/// Walk parent links until a chapter whose parent is not itself a chapter.
fn top_chapter<'a>(
    mut chapter_id: &'a str,
    parents: &HashMap<&'a str, &'a str>,
    chapter_ids: &HashSet<&'a str>,
) -> Option<&'a str> {
    for _ in 0..MAX_CHAPTER_DEPTH {
        let parent = parents.get(chapter_id)?;
        if !chapter_ids.contains(parent) {
            return Some(chapter_id);
        }
        chapter_id = parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value as JsonValue};

    fn node(value: JsonValue) -> BookTaskNode {
        serde_json::from_value(value).unwrap()
    }

    fn finished(rows: JsonValue) -> Vec<FinishedTask> {
        serde_json::from_value(rows).unwrap()
    }

    fn job_record(
        content_id: &str,
        job_id: &str,
        book_id: &str,
        course_id: &str,
    ) -> up366_store::JobRecord {
        up366_store::JobRecord {
            course_id: up366_core::CourseId::from(course_id),
            job_id: up366_core::JobId::from(job_id),
            content_id: Some(content_id.to_owned()),
            book_id: Some(book_id.to_owned()),
            chapter_id: None,
            paper_id: None,
            name: "课文跟读".to_owned(),
            order_index: 0,
            raw_json: "{}".to_owned(),
            fetched_at: 0,
        }
    }

    /// Two chapters, one nested chapter, a recording task, a vocabulary task
    /// and a layout node that is not an assignment.
    fn sample_nodes() -> Vec<BookTaskNode> {
        vec![
            node(json!({ "cid": "T1", "pid": "0", "nodeName": "Unit 1", "nodeType": 1 })),
            node(json!({ "cid": "T1a", "pid": "T1", "nodeName": "Lesson 1", "nodeType": 1 })),
            node(json!({ "cid": "T2", "pid": "0", "nodeName": "Unit 2", "nodeType": 1 })),
            node(json!({ "cid": "T3", "pid": "0", "nodeName": "Unit 3", "nodeType": 1 })),
            node(
                json!({ "cid": "TK1", "pid": "T1a", "nodeName": "课文跟读", "nodeType": 2, "taskType": 1 }),
            ),
            node(
                json!({ "cid": "TK2", "pid": "T2", "nodeName": "Task1", "nodeType": 2, "taskType": 2 }),
            ),
            node(json!({ "cid": "CAP", "pid": "", "nodeName": "caption", "nodeType": 0 })),
        ]
    }

    #[test]
    fn assignments_are_grouped_under_their_top_chapter_and_classified() {
        let finished = finished(json!([{ "taskId": "TK1", "score": 88.5 }]));
        let jobs = vec![job_record("TK1", "JOB-1", "BOOK-1", "COURSE-1")];
        let (chapters, progress, total) =
            chapters_from_nodes(&sample_nodes(), &finished, &jobs, "BOOK-1", "COURSE-1", 1);

        assert_eq!((progress, total), (1, 2));
        let titles: Vec<_> = chapters.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, vec!["Unit 1", "Unit 2", "Unit 3"]);

        let recording = &chapters[0].jobs[0];
        assert_eq!(recording.job_id, "TK1");
        assert_eq!(recording.kind, JobKind::Ordinary);
        assert!(recording.read_aloud);
        assert_eq!(recording.status, StatusView::Finished { score: Some(88.5) });
        assert_eq!(recording.score, Some(88.5));
        assert_eq!(recording.readaloud_job_id.as_deref(), Some("JOB-1"));
        assert!(!recording.readaloud_book);

        let vocabulary = &chapters[1].jobs[0];
        assert_eq!(vocabulary.job_id, "TK2");
        assert_eq!(vocabulary.kind, JobKind::WordPk);
        assert!(!vocabulary.read_aloud);
        assert_eq!(vocabulary.status, StatusView::Pending);
        // A vocabulary task is not a recording duty, so it never carries one.
        assert_eq!(vocabulary.readaloud_job_id, None);

        // A chapter with no assignments stays visible.
        assert!(chapters[2].jobs.is_empty());
    }

    #[test]
    fn an_empty_list_has_no_chapters() {
        let (chapters, progress, total) = chapters_from_nodes(&[], &[], &[], "B", "C", 1);
        assert!(chapters.is_empty());
        assert_eq!((progress, total), (0, 0));
    }

    #[test]
    fn the_recording_job_is_joined_on_the_content_id() {
        let finished = finished(json!([]));
        let jobs = vec![
            job_record("TK1", "JOB-OTHER", "OTHER-BOOK", "OTHER-COURSE"),
            job_record("TK1", "JOB-SAME-BOOK", "BOOK-1", "OTHER-COURSE"),
            job_record("TK1", "JOB-BEST", "BOOK-1", "COURSE-1"),
            job_record("UNRELATED", "JOB-UNRELATED", "BOOK-1", "COURSE-1"),
        ];
        let (chapters, _, _) =
            chapters_from_nodes(&sample_nodes(), &finished, &jobs, "BOOK-1", "COURSE-1", 1);
        let recording = &chapters[0].jobs[0];

        assert_eq!(recording.job_id, "TK1");
        assert_eq!(recording.readaloud_job_id.as_deref(), Some("JOB-BEST"));
        // An ordinary book's task is not a course-book page.
        assert!(!recording.readaloud_book);
    }

    #[test]
    fn a_course_book_recording_is_submittable_without_a_job() {
        let finished = finished(json!([]));
        let (chapters, _, _) =
            chapters_from_nodes(&sample_nodes(), &finished, &[], "BOOK-1", "COURSE-1", 2);
        let recording = &chapters[0].jobs[0];

        assert!(recording.read_aloud);
        assert_eq!(recording.readaloud_job_id, None);
        assert!(recording.readaloud_book);
    }

    fn book(value: JsonValue) -> Book {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn shelf_includes_course_books_and_keeps_purchased_metadata() {
        let purchased = vec![book(json!({
            "bookId": "B-1",
            "bookName": "探索·发现",
            "goodsImg": "cover-1",
            "packType": 6
        }))];
        let assigned = vec![
            (
                "9073115".to_owned(),
                book(json!({ "bookId": "B-1", "bookName": "探索·发现" })),
            ),
            (
                "9222783".to_owned(),
                book(json!({ "bookId": "B-2", "bookName": "新概念英语1", "bookType": 2 })),
            ),
            (
                "9222783".to_owned(),
                book(json!({ "bookName": "没有 id 的行" })),
            ),
        ];

        let items = shelf_items(&purchased, &assigned);

        // The duplicate keeps the purchased row, and the id-less row is dropped
        // because the frontend keys cards on the id.
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].book_id, "B-1");
        assert_eq!(items[0].cover_url.as_deref(), Some("cover-1"));
        // A purchased row without `bookType` keeps the PC client's default.
        assert_eq!(items[0].book_type, 1);
        // The assigned copy still supplies the course for the detail tree.
        assert_eq!(items[0].course_id.as_deref(), Some("9073115"));
        assert_eq!(items[1].book_id, "B-2");
        assert_eq!(items[1].name, "新概念英语1");
        assert_eq!(items[1].book_type, 2);
        assert_eq!(items[1].course_id.as_deref(), Some("9222783"));
    }
}
