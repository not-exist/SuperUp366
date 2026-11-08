//! Book commands: shelf and book detail.
//!
//! Scope note: the chapter **tree** is read-only metadata (titles and ids).
//! Individual exercises inside a book are separate tasks whose payload shape is
//! not confirmed yet (see `docs/protocol/to-probe.md`), so chapters are listed
//! without inventing task rows. A task that shows up in the merged job list is
//! still fully usable.

use serde::Serialize;
use up366_protocol::{adapters, dto::BookTree};

use crate::error::CommandResult;
use crate::session::AppState;

/// One shelf entry.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookListItem {
    pub book_id: String,
    pub name: String,
    pub subject: String,
    pub cover_url: Option<String>,
    /// `packType == 9` books use the newer reader.
    pub is_2025_reader: bool,
}

/// One chapter of a book.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChapterView {
    pub chapter_id: String,
    pub title: String,
    /// Whether the course plan marks this chapter as closed.
    pub closed: Option<bool>,
    /// Child chapter ids, when the shelf response nests them.
    pub children: Vec<String>,
}

/// Book detail: metadata plus its chapters.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BookDetailView {
    pub book_id: String,
    pub name: String,
    pub subject: String,
    pub cover_url: Option<String>,
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

/// One book with its chapter list.
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

    Ok(BookDetailView {
        book_id: book.id().unwrap_or_default().to_owned(),
        name: book.title().to_owned(),
        subject: book.subject_id.clone().unwrap_or_default(),
        cover_url: book.cover().map(str::to_owned),
        chapters: chapters_from_tree(&tree),
    })
}

/// Chapter views in upstream order.
fn chapters_from_tree(tree: &BookTree) -> Vec<ChapterView> {
    tree.chapters
        .iter()
        .map(|chapter| ChapterView {
            chapter_id: chapter.id().unwrap_or_default().to_owned(),
            title: chapter.title().to_owned(),
            closed: chapter.is_close,
            children: child_ids(chapter),
        })
        .collect()
}

/// Child ids from the tree's nested chapter list, if present.
fn child_ids(chapter: &up366_protocol::BookChapter) -> Vec<String> {
    let Some(children) = chapter.dir_chapters() else {
        return Vec::new();
    };
    children
        .iter()
        .filter_map(|child| {
            child
                .get("chapterId")
                .or_else(|| child.get("id"))
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> BookTree {
        BookTree::from_value(serde_json::json!({
            "version": "v42",
            "chapters": [
                {
                    "chapterId": "C-1",
                    "chapterName": "Unit 1",
                    "resourceList": { "dirChapters": [
                        { "chapterId": "C-1a", "chapterName": "Lesson 1" },
                        { "chapterId": "C-1b", "chapterName": "Lesson 2" }
                    ]}
                },
                { "chapterId": "C-2", "chapterName": "Unit 2", "isClose": true }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn chapters_keep_upstream_order_and_expose_children() {
        let views = chapters_from_tree(&tree());
        let ids: Vec<_> = views.iter().map(|view| view.chapter_id.as_str()).collect();
        assert_eq!(ids, vec!["C-1", "C-2"]);

        assert_eq!(views[0].children, vec!["C-1a", "C-1b"]);
        assert_eq!(views[1].children, Vec::<String>::new());
    }

    #[test]
    fn closed_flag_is_read_from_the_course_plan_response() {
        let views = chapters_from_tree(&tree());
        assert_eq!(views[1].closed, Some(true));
        assert_eq!(views[0].closed, None);
    }
}
