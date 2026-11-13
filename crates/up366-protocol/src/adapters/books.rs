//! Book / chapter tree / plan adapters.

use serde_json::json;

use crate::adapters::{decode_items, endpoint};
use crate::client::Client;
use crate::codec;
use crate::dto::{self, Book, BookPlanEntry, BookTree};
use crate::error::Result;

/// `data` array from `/client/books/buy-book` (no request body).
pub async fn books(client: &Client) -> Result<Vec<Book>> {
    let endpoint = endpoint("books")?;
    let data = client.call(endpoint, &[]).await?;
    let items = codec::into_array(data, endpoint.name)?;
    decode_items(items, endpoint.name)
}

/// Books attached to the student's courses, each with the course that assigned
/// it.
///
/// `/client/books/buy-book` only returns purchased goods. Course books such as
/// 新概念英语 are assigned by the class and only appear in `data[].books` of
/// `/client/student/course/list`, so the shelf has to read both lists. The
/// course id is kept because `books/tree` only returns some assigned books
/// when the request carries it.
pub async fn course_books(client: &Client) -> Result<Vec<(String, Book)>> {
    let endpoint = endpoint("course_books")?;
    let data = client.call(endpoint, &[]).await?;
    let courses = codec::into_array(data, endpoint.name)?;
    let mut books = Vec::new();
    for course in courses {
        let Some(items) = course.get("books").and_then(|value| value.as_array()) else {
            continue;
        };
        let Some(course_id) = course.get("courseId").and_then(codec::value_to_string) else {
            continue;
        };
        for book in decode_items(items.clone(), endpoint.name)? {
            books.push((course_id.clone(), book));
        }
    }
    Ok(books)
}

/// `data` object from `/client/v2/book/info`.
///
/// `book_type` must be the shelf's `bookType`: a class-assigned book (type 2)
/// asked for as type 1 answers `code: 0` with no `data`, which is exactly the
/// protocol drift this parameter prevents.
pub async fn book_info(client: &Client, book_id: &str, book_type: i64) -> Result<Book> {
    let endpoint = endpoint("book_info")?;
    let data = client
        .call(
            endpoint,
            &[
                ("bookId", json!(book_id)),
                ("bookType", json!(book_type)),
                ("originalBookId", json!("")),
            ],
        )
        .await?;
    dto::decode(codec::require_data(data, endpoint.name)?, endpoint.name)
}

/// `data` tree from `/client/books/tree`. `version=""` forces a fresh tree.
///
/// `course_id` is needed for some class-assigned books: 新概念英语1 answers an
/// empty tree unless the request carries the assigning course. It is optional
/// because sending an unrelated course can make other books answer empty.
pub async fn book_tree(
    client: &Client,
    book_id: &str,
    book_type: i64,
    course_id: Option<&str>,
) -> Result<BookTree> {
    let endpoint = endpoint("book_tree")?;
    let mut params = vec![
        ("bookId", json!(book_id)),
        ("version", json!("")),
        ("bookType", json!(book_type)),
    ];
    if let Some(course_id) = course_id {
        params.push(("courseId", json!(course_id)));
    }
    let data = client.call(endpoint, &params).await?;
    BookTree::from_value(codec::require_data(data, endpoint.name)?)
}

/// `data` array from `/client/book/plan`.
///
/// Course id `0` or negative is not sent at all, matching the PC client: the
/// plan is only meaningful for a real course.
pub async fn book_plan(
    client: &Client,
    course_id: &str,
    book_id: &str,
) -> Result<Vec<BookPlanEntry>> {
    if course_id
        .parse::<i64>()
        .map(|course| course <= 0)
        .unwrap_or(false)
    {
        return Ok(Vec::new());
    }
    let endpoint = endpoint("book_plan")?;
    let data = client
        .call(
            endpoint,
            &[
                ("courseId", json!(course_id)),
                ("bookId", json!(book_id)),
                ("bookType", json!(1)),
            ],
        )
        .await?;
    let items = codec::into_array(data, endpoint.name)?;
    decode_items(items, endpoint.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::test_support::{client, ok};
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn books_posts_empty_body_and_decodes_shelf() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/books/buy-book");
            then.status(200).json_body(ok(json!([{
                "bookId": "BOOK-1",
                "goodsId": 99001,
                "bookName": "英语必修一",
                "goodsImg": "https://cdn/cover.jpg",
                "goodsVerticalImg": "https://cdn/vertical.jpg",
                "packType": 9,
                "startDate": "2026-09-01",
                "endDate": "2027-07-01",
                "fileId": "F-1"
            }])));
        });

        let client = client(&server);
        let books = books(&client).await.unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id(), Some("BOOK-1"));
        assert_eq!(books[0].title(), "英语必修一");
        assert_eq!(books[0].cover(), Some("https://cdn/cover.jpg"));
        assert!(books[0].is_2025_reader());
        assert_eq!(books[0].raw["goodsId"], json!(99001));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn course_books_flattens_every_course_and_skips_courses_without_books() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/student/course/list");
            then.status(200).json_body(ok(json!([
                { "courseId": 1, "books": [
                    { "bookId": "B-1", "bookName": "新概念英语1", "bookType": 2 },
                    { "bookId": "B-2", "bookName": "语法专项练习" }
                ]},
                { "courseId": 2, "books": [
                    { "bookId": "B-1", "bookName": "新概念英语1" }
                ]},
                { "courseId": 3 }
            ])));
        });

        let client = client(&server);
        let books = course_books(&client).await.unwrap();

        // Flattening keeps the duplicate: the shelf dedupes across both sources.
        assert_eq!(books.len(), 3);
        assert_eq!(books[0].0, "1");
        assert_eq!(books[0].1.id(), Some("B-1"));
        assert_eq!(books[0].1.title(), "新概念英语1");
        assert_eq!(books[0].1.raw["bookType"], json!(2));
        assert_eq!(books[2].0, "2");
        assert_eq!(books[2].1.id(), Some("B-1"));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn book_info_sends_the_shelf_book_type_and_empty_original_id() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/v2/book/info")
                .body_contains("bookId=BOOK-2")
                .body_contains("bookType=2")
                .body_contains("originalBookId=");
            then.status(200).json_body(ok(
                json!({ "bookId": "BOOK-2", "bookName": "数学", "bookType": 2, "packType": 7 }),
            ));
        });

        let client = client(&server);
        let book = book_info(&client, "BOOK-2", 2).await.unwrap();
        assert_eq!(book.book_type(), 2);
        assert_eq!(book.title(), "数学");
        assert!(!book.is_2025_reader());
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn book_tree_decodes_version_and_chapters() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/books/tree")
                .body_contains("bookId=BOOK-1")
                .body_contains("version=")
                .body_contains("bookType=2")
                .body_contains("courseId=COURSE-9");
            then.status(200).json_body(ok(json!({
                "version": "v42",
                "book": { "configs": { "customFlag": 1 } },
                "chapters": [{ "id": "C-1", "name": "Unit 1", "type": 2 }],
                "pages": [],
                "tasks": []
            })));
        });

        let client = client(&server);
        let tree = book_tree(&client, "BOOK-1", 2, Some("COURSE-9"))
            .await
            .unwrap();

        assert_eq!(tree.version.as_deref(), Some("v42"));
        assert!(tree.has_custom_chapters());
        assert_eq!(tree.chapter("C-1").unwrap().title(), "Unit 1");
        assert_eq!(tree.chapter("C-1").unwrap().kind, Some(2));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn book_plan_skips_the_request_for_non_positive_courses() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/client/book/plan");
            then.status(200).json_body(ok(json!([])));
        });

        let client = client(&server);
        assert!(book_plan(&client, "0", "BOOK-1").await.unwrap().is_empty());
        assert!(book_plan(&client, "-1", "BOOK-1").await.unwrap().is_empty());
        mock.assert_hits(0);
    }

    #[tokio::test]
    async fn book_plan_decodes_closed_chapters() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/client/book/plan")
                .body_contains("courseId=9222783")
                .body_contains("bookId=BOOK-1")
                .body_contains("bookType=1");
            then.status(200).json_body(ok(json!([
                { "chapterId": "C-1", "chapterName": "Unit 1", "isClose": 1 },
                { "chapterId": "C-2", "chapterName": "Unit 2", "isClose": 0 }
            ])));
        });

        let client = client(&server);
        let plan = book_plan(&client, "9222783", "BOOK-1").await.unwrap();

        assert_eq!(plan.len(), 2);
        assert!(plan[0].is_closed());
        assert!(!plan[1].is_closed());
        mock.assert_hits(1);
    }
}
