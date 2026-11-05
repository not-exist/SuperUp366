"""Minimal up366 (Tianxuewang) API client."""

from .book import Book, Chapter
from .client import (
    Job,
    Up366,
    Up366APIError,
    Up366AuthError,
    Up366Error,
    Up366HTTPError,
    Up366RequestError,
    Up366ResponseError,
    login,
    up366,
)
from .job import JobStatus
from .wrongnote import (
    WrongBook,
    WrongCourse,
    WrongKnowledge,
    WrongNotePage,
    WrongQuestion,
    WrongRank,
    WrongSubject,
    WrongSubjectStat,
)

__all__ = [
    "Book",
    "Chapter",
    "Job",
    "JobStatus",
    "Up366",
    "Up366APIError",
    "Up366AuthError",
    "Up366Error",
    "Up366HTTPError",
    "Up366RequestError",
    "Up366ResponseError",
    "WrongBook",
    "WrongCourse",
    "WrongKnowledge",
    "WrongNotePage",
    "WrongQuestion",
    "WrongRank",
    "WrongSubject",
    "WrongSubjectStat",
    "login",
    "up366",
]
