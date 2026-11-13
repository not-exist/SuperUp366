//! Book commands: shelf and book detail.
//!
//! A book's tree is three flat arrays: `chapters` (type 1), `pages` (type 2)
//! and `tasks` (type 3). Each page carries the task it belongs to in
//! `taskId`, and `/client/book/finished-task/list` reports finished tasks by
//! the same id, so progress is a set intersection — no per-page request.
//!
//! The detail view groups pages under their **top-level** chapter: nested
//! chapters are only navigation, and a book like 《探索·发现》 has 2535 leaf
//! chapters with one page each under two top-level chapters.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value as JsonValue;
use up366_core::JobKind;
use up366_protocol::{
    adapters,
    dto::{Book, BookTree, FinishedTask},
};

use crate::error::CommandResult;
use crate::session::AppState;
use crate::state::StatusView;

/// How many parent links are followed before a chapter chain is treated as
/// broken. Real trees are three levels deep; the bound only guards cycles.
const MAX_CHAPTER_DEPTH: usize = 16;

/// One shelf entry. Deliberately metadata only: computing progress needs the
/// whole tree (megabytes for a large book), so it is only done on the detail
/// page.
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
    /// The course that assigned this book, when one did. `books/tree` returns
    /// some assigned books (新概念英语1) only when the course is passed.
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
    /// Assignments that appear in the finished-task list.
    pub progress: usize,
    /// Assignments the tree lists.
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
    let mut tree = adapters::book_tree(&client, &book_id, book_type, None)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    // 新概念英语1 (and other course content books) answer an empty tree unless
    // the assigning course rides along; retry only when the first answer has
    // no usable content, because an unrelated course empties some other books.
    if !tree_has_content(&tree) {
        if let Some(course_id) = course_id.as_deref() {
            tree = adapters::book_tree(&client, &book_id, book_type, Some(course_id))
                .await
                .map_err(|error| state.session_error(&account_id, &client, error))?;
        }
    }
    let finished = adapters::finished_tasks(&client, &book_id)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;

    let (chapters, progress, total) = chapters_from_tree(&tree, &finished);

    Ok(BookDetailView {
        book_id: book.id().unwrap_or_default().to_owned(),
        name: book.title().to_owned(),
        subject: book.subject_id.clone().unwrap_or_default(),
        cover_url: book.cover().map(str::to_owned),
        is_2025_reader: book.is_2025_reader(),
        progress,
        total,
        teacher_name: None,
        chapters,
    })
}

/// Whether the tree carries rows the detail page can render.
///
/// Chapters alone are an empty page: jobs come from `tasks` or from type-2
/// content pages, so either one has to be present.
fn tree_has_content(tree: &BookTree) -> bool {
    !tree.pages.is_empty() || !tree.tasks.is_empty()
}

/// Group every task under the top-level chapter that owns it.
///
/// A book's assignments live in `tasks` (type 3); a page (type 2) only links to
/// one through `taskId`. Building from pages alone misses every task whose
/// chapter has no page (the "测验" nodes), which is what made large books look
/// incomplete. Tasks are authoritative, and a page contributes its display name
/// when one links to the task.
///
/// Order is the upstream array order for chapters, tasks and pages; nothing is
/// re-sorted. Returns the chapters plus `(progress, total)`; a task counts as
/// finished when its id appears in the finished-task list.
fn chapters_from_tree(
    tree: &BookTree,
    finished: &[FinishedTask],
) -> (Vec<ChapterView>, usize, usize) {
    // chapter id -> parent id ("" for roots). Keeping the ids in one map lets
    // both the root test and the ancestor walk share the same lookup.
    let parents: HashMap<&str, &str> = tree
        .chapters
        .iter()
        .filter_map(|chapter| Some((chapter.id()?, chapter.parent_id.as_deref().unwrap_or(""))))
        .collect();
    let chapter_ids: HashSet<&str> = parents.keys().copied().collect();

    let mut chapters: Vec<ChapterView> = Vec::new();
    let mut top_index: HashMap<&str, usize> = HashMap::new();
    let mut chapter_titles: HashMap<&str, &str> = HashMap::new();
    for chapter in &tree.chapters {
        let Some(id) = chapter.id() else { continue };
        chapter_titles.insert(id, chapter.title());
        if chapter_ids.contains(chapter.parent_id.as_deref().unwrap_or("")) {
            continue;
        }
        top_index.insert(id, chapters.len());
        chapters.push(ChapterView {
            chapter_id: id.to_owned(),
            title: chapter.title().to_owned(),
            jobs: Vec::new(),
        });
    }

    // taskId -> display name from the page that links to it.
    let mut page_names: HashMap<&str, &str> = HashMap::new();
    for page in &tree.pages {
        let Some(page) = page.as_object() else {
            continue;
        };
        let Some(task_id) = page
            .get("taskId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        if let Some(name) = page.get("name").and_then(JsonValue::as_str) {
            page_names.entry(task_id).or_insert(name);
        }
    }

    // taskId -> score. A task that is absent from this map is unfinished.
    let mut scores: HashMap<&str, Option<f64>> = HashMap::new();
    for task in finished {
        if let Some(task_id) = task.task_id.as_deref() {
            scores.entry(task_id).or_insert(task.score);
        }
    }

    let mut progress = 0;
    let mut total = 0;
    let mut seen: HashSet<&str> = HashSet::new();

    // Tasks are the authoritative assignment list.
    for task in &tree.tasks {
        let Some(task) = task.as_object() else {
            continue;
        };
        let Some(task_id) = task
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        if !seen.insert(task_id) {
            continue;
        }
        total += 1;

        let record = scores.get(task_id);
        let status = match record {
            Some(score) => {
                progress += 1;
                StatusView::Finished { score: *score }
            }
            None => StatusView::Pending,
        };

        let parent_id = task
            .get("parentId")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        let Some(top) = top_chapter(parent_id, &parents, &chapter_ids) else {
            continue;
        };
        let Some(&index) = top_index.get(top) else {
            continue;
        };
        let name = page_names
            .get(task_id)
            .copied()
            .filter(|name| !name.is_empty())
            .or_else(|| chapter_titles.get(parent_id).copied())
            .filter(|name| !name.is_empty())
            .unwrap_or("任务");
        chapters[index].jobs.push(BookJobView {
            job_id: task_id.to_owned(),
            name: name.to_owned(),
            kind: JobKind::Ordinary,
            status,
            score: record.copied().flatten(),
            deadline: None,
            read_aloud: false,
        });
    }

    // Type-2 course books carry no `tasks`: the content pages themselves are
    // the assignments (`isContent=1`, `paperId`), and the official reader keys
    // their finished rows on the page id. Only use them when `tasks` is empty,
    // so navigation pages never become rows in ordinary books.
    if tree.tasks.is_empty() {
        for page in &tree.pages {
            let Some(page) = page.as_object() else {
                continue;
            };
            let Some(page_id) = page
                .get("id")
                .and_then(JsonValue::as_str)
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            let is_content = page
                .get("isContent")
                .and_then(JsonValue::as_i64)
                .is_some_and(|value| value != 0)
                || page
                    .get("paperId")
                    .and_then(JsonValue::as_str)
                    .is_some_and(|id| !id.is_empty());
            if !is_content || !seen.insert(page_id) {
                continue;
            }
            total += 1;

            let record = scores.get(page_id);
            let status = match record {
                Some(score) => {
                    progress += 1;
                    StatusView::Finished { score: *score }
                }
                None => StatusView::Pending,
            };

            let parent_id = page
                .get("parentId")
                .and_then(JsonValue::as_str)
                .unwrap_or("");
            let Some(top) = top_chapter(parent_id, &parents, &chapter_ids) else {
                continue;
            };
            let Some(&index) = top_index.get(top) else {
                continue;
            };
            let name = page
                .get("name")
                .and_then(JsonValue::as_str)
                .filter(|name| !name.is_empty())
                .or_else(|| chapter_titles.get(parent_id).copied())
                .filter(|name| !name.is_empty())
                .unwrap_or("任务");
            chapters[index].jobs.push(BookJobView {
                job_id: page_id.to_owned(),
                name: name.to_owned(),
                kind: JobKind::Ordinary,
                status,
                score: record.copied().flatten(),
                deadline: None,
                read_aloud: name.contains("跟读") || name.contains("朗读"),
            });
        }
    }

    // Some older payloads only put the task id on the page.
    for page in &tree.pages {
        let Some(page) = page.as_object() else {
            continue;
        };
        let Some(task_id) = page
            .get("taskId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
        else {
            continue;
        };
        if !seen.insert(task_id) {
            continue;
        }
        total += 1;

        let record = scores.get(task_id);
        let status = match record {
            Some(score) => {
                progress += 1;
                StatusView::Finished { score: *score }
            }
            None => StatusView::Pending,
        };

        let parent_id = page
            .get("parentId")
            .and_then(JsonValue::as_str)
            .unwrap_or("");
        let Some(top) = top_chapter(parent_id, &parents, &chapter_ids) else {
            continue;
        };
        let Some(&index) = top_index.get(top) else {
            continue;
        };
        let name = page
            .get("name")
            .and_then(JsonValue::as_str)
            .filter(|name| !name.is_empty())
            .or_else(|| chapter_titles.get(parent_id).copied())
            .unwrap_or("任务");
        chapters[index].jobs.push(BookJobView {
            job_id: task_id.to_owned(),
            name: name.to_owned(),
            kind: JobKind::Ordinary,
            status,
            score: record.copied().flatten(),
            deadline: None,
            read_aloud: false,
        });
    }

    (chapters, progress, total)
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
    use serde_json::json;

    fn tree(value: JsonValue) -> BookTree {
        BookTree::from_value(value).unwrap()
    }

    fn finished(rows: JsonValue) -> Vec<FinishedTask> {
        serde_json::from_value(rows).unwrap()
    }

    /// Two levels below the root, one page without a task, one empty chapter
    /// and a finished task that belongs to no page.
    fn sample_tree() -> BookTree {
        tree(json!({
            "chapters": [
                { "id": "T1", "parentId": "", "name": "Unit 1", "type": 1 },
                { "id": "T1a", "parentId": "T1", "name": "Lesson 1", "type": 1 },
                { "id": "T1b", "parentId": "T1a", "name": "Part A", "type": 1 },
                { "id": "T2", "parentId": "", "name": "Unit 2", "type": 1 },
                { "id": "T3", "parentId": "", "name": "Unit 3", "type": 1 }
            ],
            "pages": [
                { "id": "P1", "parentId": "T1b", "name": "Page 1", "taskId": "TK1", "type": 2 },
                { "id": "P2", "parentId": "T1a", "name": "Page 2", "taskId": "TK2", "type": 2 },
                { "id": "P3", "parentId": "T1", "name": "Page 3", "type": 2 },
                { "id": "P4", "parentId": "T2", "name": "Page 4", "taskId": "TK3", "type": 2 }
            ]
        }))
    }

    #[test]
    fn top_chapters_collect_pages_from_every_level_below_them() {
        let (chapters, _, _) = chapters_from_tree(&sample_tree(), &[]);

        let titles: Vec<_> = chapters
            .iter()
            .map(|chapter| chapter.title.as_str())
            .collect();
        assert_eq!(titles, vec!["Unit 1", "Unit 2", "Unit 3"]);

        let names = |index: usize| -> Vec<_> {
            chapters[index]
                .jobs
                .iter()
                .map(|job| job.name.as_str())
                .collect()
        };
        // Array order, and both the child and the grandchild page land here.
        assert_eq!(names(0), vec!["Page 1", "Page 2"]);
        assert_eq!(names(1), vec!["Page 4"]);
        // A chapter without pages stays visible with an empty job list.
        assert!(chapters[2].jobs.is_empty());
    }

    #[test]
    fn finished_tasks_map_to_pages_and_drive_progress() {
        let finished = finished(json!([
            { "taskId": "TK1", "score": 88.5 },
            { "taskId": "TK9", "score": 100.0 }
        ]));
        let (chapters, progress, total) = chapters_from_tree(&sample_tree(), &finished);

        // P3 has no taskId and TK9 has no page: neither counts.
        assert_eq!((progress, total), (1, 3));

        let page_one = &chapters[0].jobs[0];
        assert_eq!(page_one.job_id, "TK1");
        assert_eq!(page_one.status, StatusView::Finished { score: Some(88.5) });
        assert_eq!(page_one.score, Some(88.5));
        assert_eq!(page_one.kind, JobKind::Ordinary);
        assert_eq!(page_one.deadline, None);
        assert!(!page_one.read_aloud);

        let page_two = &chapters[0].jobs[1];
        assert_eq!(page_two.status, StatusView::Pending);
        assert_eq!(page_two.score, None);
    }

    #[test]
    fn an_empty_tree_decodes_to_empty_chapters() {
        let (chapters, progress, total) = chapters_from_tree(&tree(json!({})), &[]);
        assert!(chapters.is_empty());
        assert_eq!((progress, total), (0, 0));
    }

    #[test]
    fn tasks_without_a_page_are_still_listed() {
        // The large reader books put most assignments in `tasks`; only a few
        // have a page that links to them.
        let tree = tree(json!({
            "chapters": [
                { "id": "T1", "parentId": "", "name": "Unit 1", "type": 1 },
                { "id": "T1a", "parentId": "T1", "name": "测验", "type": 1 }
            ],
            "pages": [
                { "id": "P1", "parentId": "T1", "name": "阅读", "taskId": "TK1", "type": 2 }
            ],
            "tasks": [
                { "id": "TK1", "parentId": "T1", "type": 3, "taskType": "1" },
                { "id": "TK2", "parentId": "T1a", "type": 3, "taskType": "1" }
            ]
        }));
        let finished = finished(json!([{ "taskId": "TK2", "score": 100.0 }]));

        let (chapters, progress, total) = chapters_from_tree(&tree, &finished);

        assert_eq!((progress, total), (1, 2));
        let jobs = &chapters[0].jobs;
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].job_id, "TK1");
        assert_eq!(jobs[0].name, "阅读");
        assert_eq!(jobs[0].status, StatusView::Pending);
        assert_eq!(jobs[1].job_id, "TK2");
        assert_eq!(jobs[1].name, "测验");
        assert_eq!(jobs[1].status, StatusView::Finished { score: Some(100.0) });
    }

    #[test]
    fn content_pages_are_the_jobs_of_a_course_book_without_tasks() {
        // Type-2 books have chapters and content pages but no `tasks`; the
        // page id is what the reader uses, and finished rows key on it.
        let tree = tree(json!({
            "chapters": [
                { "id": "L1", "parentId": "", "name": "Lesson 1", "type": 1 },
                { "id": "L2", "parentId": "", "name": "Lesson 2", "type": 1 }
            ],
            "pages": [
                { "id": "P1", "parentId": "L1", "name": "课文跟读", "isContent": 1, "paperId": "PAPER-1" },
                { "id": "P2", "parentId": "L1", "name": "课文详解" },
                { "id": "P3", "parentId": "L2", "name": "课文跟读", "paperId": "PAPER-3" }
            ]
        }));
        let finished = finished(json!([{ "taskId": "P1", "score": 88.0 }]));

        let (chapters, progress, total) = chapters_from_tree(&tree, &finished);

        assert_eq!((progress, total), (1, 2));
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].jobs[0].job_id, "P1");
        assert_eq!(chapters[0].jobs[0].name, "课文跟读");
        assert!(chapters[0].jobs[0].read_aloud);
        assert_eq!(
            chapters[0].jobs[0].status,
            StatusView::Finished { score: Some(88.0) }
        );
        assert_eq!(chapters[1].jobs[0].job_id, "P3");
        assert_eq!(chapters[1].jobs[0].status, StatusView::Pending);
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
