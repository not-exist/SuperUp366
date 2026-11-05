from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

BodyKind = Literal["form", "query", "json", "none"]


@dataclass(frozen=True)
class Endpoint:
    name: str
    base_url: str
    path: str
    method: str = "POST"
    body: BodyKind = "form"
    fallback: str | None = None
    write: bool = False


ENDPOINTS = {
    "courses": Endpoint(
        name="courses",
        base_url="https://course-api.up366.cn",
        path="/client/course/list-of-student/v2",
    ),
    "jobs": Endpoint(
        name="jobs",
        base_url="https://studytask-api.up366.cn",
        path="/client/student/course/job",
    ),
    "job_info": Endpoint(
        name="job_info",
        base_url="https://studytask-api.up366.cn",
        path="/client/student/job/info",
    ),
    "bucket_words": Endpoint(
        name="bucket_words",
        base_url="https://words-v2-api.up366.cn",
        path="/client/sync/teaching/bucket/detail-info",
    ),
    "word_pk_submit": Endpoint(
        name="word_pk_submit",
        base_url="https://wordsbtl-api.up366.cn",
        path="/client/wordsbtl/student/submit",
        fallback="GET",
        write=True,
    ),
    # --- ordinary book APIs (book-api / studytask-api) ---
    "books": Endpoint(
        name="books",
        base_url="https://book-api.up366.cn",
        path="/client/books/buy-book",
        body="none",
    ),
    "book_info": Endpoint(
        name="book_info",
        base_url="https://book-api.up366.cn",
        path="/client/v2/book/info",
    ),
    "book_tree": Endpoint(
        name="book_tree",
        base_url="https://book-api.up366.cn",
        path="/client/books/tree",
    ),
    "custom_chapters": Endpoint(
        name="custom_chapters",
        base_url="https://book-api.up366.cn",
        path="/client/custom/book/chapters",
    ),
    "personalized_chapters": Endpoint(
        name="personalized_chapters",
        base_url="https://book-api.up366.cn",
        path="/client/chapters/personalized/list",
    ),
    "offline_books": Endpoint(
        name="offline_books",
        base_url="https://book-api.up366.cn",
        path="/front/offline/download/books",
        body="none",
    ),
    "book_plan": Endpoint(
        name="book_plan",
        base_url="https://studytask-api.up366.cn",
        path="/client/book/plan",
    ),
    # --- ordinary job APIs ---
    "finished_tasks": Endpoint(
        name="finished_tasks",
        base_url="https://dsb-api.up366.cn",
        path="/client/book/finished-task/list",
    ),
    "answer_detail": Endpoint(
        name="answer_detail",
        base_url="https://dsb-api.up366.cn",
        path="/client/student/task/answer/detail/v2",
    ),
    "answer_detail_v1": Endpoint(
        name="answer_detail_v1",
        base_url="https://dsb-api.up366.cn",
        path="/client/student/task/answer/detail",
        method="GET",
        body="query",
    ),
    "test_all_data": Endpoint(
        name="test_all_data",
        base_url="https://xot-api.up366.cn",
        path="/client/onlinetesting/alldata",
    ),
    "paper_zip_info": Endpoint(
        name="paper_zip_info",
        base_url="https://xot-api.up366.cn",
        path="/client/paper/zipinfo",
    ),
    "paper_detail": Endpoint(
        name="paper_detail",
        base_url="https://qlib-api.up366.cn",
        path="/client/paper/detail",
        method="GET",
        body="query",
    ),
    "chapter_questions": Endpoint(
        name="chapter_questions",
        base_url="https://book-api.up366.cn",
        path="/client/chapters/list-question-file",
    ),
    "task_linked": Endpoint(
        name="task_linked",
        base_url="https://book-api.up366.cn",
        path="/client/task/linked",
    ),
    "submit_task_score": Endpoint(
        name="submit_task_score",
        base_url="https://study-api.up366.cn",
        path="/client/task/score/submit/v2",
        write=True,
    ),
    "submit_online_test": Endpoint(
        name="submit_online_test",
        base_url="https://xot-api.up366.cn",
        path="/client/onlinetesting/submit",
        write=True,
    ),
    # --- wrong-answer book (wrongnote-api) ---
    "wrong_subjects": Endpoint(
        name="wrong_subjects",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/subject/list",
    ),
    "wrong_subject_stat": Endpoint(
        name="wrong_subject_stat",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/subject/stat",
    ),
    "wrong_books": Endpoint(
        name="wrong_books",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/book/list",
    ),
    "wrong_knowledge": Endpoint(
        name="wrong_knowledge",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/knowledge/list/v3",
    ),
    "wrong_courses": Endpoint(
        name="wrong_courses",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/course/list",
    ),
    "wrong_rank": Endpoint(
        name="wrong_rank",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/v2/rank/list",
    ),
    "wrong_killed_questions": Endpoint(
        name="wrong_killed_questions",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/kill/question/show",
    ),
    "wrong_question_show": Endpoint(
        name="wrong_question_show",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/question/show",
    ),
    "wrong_kill_paper": Endpoint(
        name="wrong_kill_paper",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/questions/kill",
    ),
    "submit_wrong_question": Endpoint(
        name="submit_wrong_question",
        base_url="https://wrongnote-api.up366.cn",
        path="/client/wrongnote/student/question/submit",
        write=True,
    ),
}
