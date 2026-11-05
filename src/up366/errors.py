"""Exception hierarchy for the up366 client.

Kept in a standalone module so API mixins can import these without creating a
circular import back through ``client`` (which imports the mixins).
"""
from __future__ import annotations

__all__ = [
    "Up366Error",
    "Up366AuthError",
    "Up366APIError",
    "Up366RequestError",
    "Up366HTTPError",
    "Up366ResponseError",
]


class Up366Error(Exception):
    """Base error for the up366 client."""


class Up366AuthError(Up366Error):
    """The server rejected the login or returned an unusable login response."""

    def __init__(self, code, message: str):
        self.code = code
        self.message = message
        super().__init__(f"up366 login failed (code={code!r}): {message}")


class Up366APIError(Up366Error):
    """The server returned a non-zero business result code."""

    def __init__(self, code, message: str):
        self.code = code
        self.message = message
        super().__init__(f"up366 api error (code={code!r}): {message}")


class Up366RequestError(Up366Error):
    """The HTTP request failed before a usable response was received."""


class Up366HTTPError(Up366Error):
    """The server returned a non-200 HTTP status."""

    def __init__(self, status_code: int, url: str, message: str):
        self.status_code = status_code
        self.url = url
        super().__init__(f"HTTP {status_code} {url}: {message}")


class Up366ResponseError(Up366Error):
    """The server response did not have the expected shape."""
