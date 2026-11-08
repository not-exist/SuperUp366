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
    dto::{BookTree, FinishedTask},
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
}

/// One page of a book, rendered as a job row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookJobView {
    /// The page's `taskId`: the id both the tree and the finished list use.
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
    /// Pages with a `taskId` that appear in the finished-task list.
    pub progress: usize,
    /// Pages with a `taskId`.
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
    Ok(adapters::books(&client)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?
        .iter()
        .map(|book| BookListItem {
            book_id: book.id().unwrap_or_default().to_owned(),
            name: book.title().to_owned(),
            subject: book.subject_id.clone().unwrap_or_default(),
            cover_url: book.cover().map(str::to_owned),
            is_2025_reader: book.is_2025_reader(),
        })
        .collect())
}

/// One book with its chapters and per-page completion.
#[tauri::command(rename_all = "snake_case")]
pub async fn book_detail(
    account_id: String,
    book_id: String,
    state: tauri::State<'_, AppState>,
) -> CommandResult<BookDetailView> {
    let (account_id, client) = state.require_client(Some(&account_id))?;
    let book = adapters::book_info(&client, &book_id)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
    let tree = adapters::book_tree(&client, &book_id)
        .await
        .map_err(|error| state.session_error(&account_id, &client, error))?;
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

/// Group every page under the top-level chapter that owns it.
///
/// Order is the upstream array order for both chapters and pages; nothing is
/// re-sorted. Returns the chapters plus `(progress, total)`, where a page only
/// counts when it carries a non-empty `taskId`.
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
    for chapter in &tree.chapters {
        let Some(id) = chapter.id() else { continue };
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

    // taskId -> score. A task that is absent from this map is unfinished.
    let mut scores: HashMap<&str, Option<f64>> = HashMap::new();
    for task in finished {
        if let Some(task_id) = task.task_id.as_deref() {
            scores.entry(task_id).or_insert(task.score);
        }
    }

    let mut progress = 0;
    let mut total = 0;
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
        chapters[index].jobs.push(BookJobView {
            job_id: task_id.to_owned(),
            name: page
                .get("name")
                .and_then(JsonValue::as_str)
                .unwrap_or("")
                .to_owned(),
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
}
