"""Small dependency-free terminal UI helpers.

The TUI is deliberately line-based instead of ``curses`` based: Python on
Windows does not ship ``curses``, and a line-based interface works in the
legacy console, Windows Terminal and PowerShell alike.
"""
from __future__ import annotations

import os
import shutil
import sys
import unicodedata
from collections.abc import Sequence
from typing import Any


class UserExit(Exception):
    """Raised when the user asks to leave the current interaction or the app."""


def display_width(text: str) -> int:
    """Return the terminal cell width of ``text`` (CJK counts as two)."""
    width = 0
    for char in text:
        if unicodedata.combining(char):
            continue
        width += 2 if unicodedata.east_asian_width(char) in "WF" else 1
    return width


def truncate(text: str, width: int) -> str:
    if width <= 0:
        return ""
    if display_width(text) <= width:
        return text
    kept: list[str] = []
    used = 0
    for char in text:
        char_width = 2 if unicodedata.east_asian_width(char) in "WF" else 1
        if used + char_width > width - 1:
            break
        kept.append(char)
        used += char_width
    return "".join(kept) + "…"


def pad(text: str, width: int, align: str = "left") -> str:
    text = truncate(text, width)
    gap = width - display_width(text)
    if gap <= 0:
        return text
    if align == "right":
        return " " * gap + text
    if align == "center":
        left = gap // 2
        return " " * left + text + " " * (gap - left)
    return text + " " * gap


class UI:
    """Rendering, input and table helpers used by the application."""

    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    CYAN = "\033[36m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    RED = "\033[31m"

    def __init__(
        self,
        *,
        color: bool | None = None,
        verbose: bool = False,
        clear_screen: bool = True,
    ):
        if color is None:
            color = bool(sys.stdout.isatty()) and os.environ.get("NO_COLOR") is None
        self.color = color
        self.verbose = verbose
        self.clear_screen = clear_screen and bool(sys.stdout.isatty())
        if self.color and os.name == "nt":
            self._enable_windows_ansi()

    @staticmethod
    def _enable_windows_ansi() -> None:
        try:
            import ctypes

            kernel32 = ctypes.windll.kernel32
            handle = kernel32.GetStdHandle(-11)
            mode = ctypes.c_uint32()
            if kernel32.GetConsoleMode(handle, ctypes.byref(mode)):
                kernel32.SetConsoleMode(handle, mode.value | 0x0004)
        except Exception:
            # ANSI is a cosmetic feature; never fail the app because of it.
            pass

    def style(self, text: str, code: str) -> str:
        if not self.color:
            return text
        return f"{code}{text}{self.RESET}"

    def line(self, text: str = "") -> None:
        print(text)

    def info(self, text: str) -> None:
        self.line(f"{self.style('[i]', self.CYAN)} {text}")

    def ok(self, text: str) -> None:
        self.line(f"{self.style('[+]', self.GREEN)} {text}")

    def warn(self, text: str) -> None:
        self.line(f"{self.style('[!]', self.YELLOW)} {text}")

    def error(self, text: str) -> None:
        self.line(f"{self.style('[x]', self.RED)} {text}")

    def title(self, text: str) -> None:
        self.line(self.style(text, self.BOLD + self.CYAN))
        self.rule("=")

    def rule(self, char: str = "-") -> None:
        width = max(20, shutil.get_terminal_size((88, 24)).columns)
        self.line(self.style(char * width, self.DIM))

    def clear(self) -> None:
        if self.clear_screen:
            sys.stdout.write("\033[2J\033[H")
            sys.stdout.flush()

    def table(
        self,
        headers: Sequence[str],
        rows: Sequence[Sequence[Any]],
        *,
        aligns: Sequence[str] | None = None,
    ) -> None:
        """Print a compact table, shrinking wide columns to the terminal."""
        headers = [str(item) for item in headers]
        data = [[str(cell) for cell in row] for row in rows]
        if not data:
            self.line(self.style("(无)", self.DIM))
            return

        widths = []
        for index, header in enumerate(headers):
            widths.append(
                max(
                    [display_width(header)]
                    + [display_width(row[index]) for row in data if index < len(row)]
                )
            )
        separator_width = 3 * max(0, len(headers) - 1)
        terminal_width = max(30, shutil.get_terminal_size((88, 24)).columns)
        while sum(widths) + separator_width > terminal_width:
            widest = max(range(len(widths)), key=lambda i: widths[i])
            if widths[widest] <= 4:
                break
            widths[widest] -= 1

        align_list = list(aligns or ["left"] * len(headers))
        header_line = " │ ".join(
            pad(header, widths[index]) for index, header in enumerate(headers)
        )
        self.line(self.style(header_line, self.DIM))
        self.line(
            self.style(
                "─┼─".join("─" * width for width in widths),
                self.DIM,
            )
        )
        for row in data:
            cells = []
            for index in range(len(headers)):
                value = row[index] if index < len(row) else ""
                cells.append(
                    pad(
                        value,
                        widths[index],
                        align_list[index] if index < len(align_list) else "left",
                    )
                )
            self.line(" │ ".join(cells))

    def ask(self, prompt: str, *, default: str = "") -> str:
        suffix = f" [{default}]" if default else ""
        try:
            value = input(f"{prompt}{suffix}: ").strip()
        except EOFError as exc:
            raise UserExit() from exc
        return value if value else default

    def ask_int(
        self,
        prompt: str,
        *,
        default: int | None = None,
        minimum: int | None = None,
        maximum: int | None = None,
    ) -> int:
        while True:
            raw = self.ask(
                prompt, default="" if default is None else str(default)
            )
            if raw == "" and default is not None:
                value = default
            else:
                try:
                    value = int(raw)
                except ValueError:
                    self.error("请输入整数。")
                    continue
            if minimum is not None and value < minimum:
                self.error(f"不能小于 {minimum}。")
                continue
            if maximum is not None and value > maximum:
                self.error(f"不能大于 {maximum}。")
                continue
            return value

    def ask_yes_no(self, prompt: str, default: bool = False) -> bool:
        suffix = "[Y/n]" if default else "[y/N]"
        while True:
            raw = self.ask(f"{prompt} {suffix}").lower()
            if raw == "":
                return default
            if raw in ("y", "yes", "是"):
                return True
            if raw in ("n", "no", "否"):
                return False
            self.error("请输入 y 或 n。")

    def pause(self, prompt: str = "按回车继续…") -> None:
        try:
            input(prompt)
        except EOFError as exc:
            raise UserExit() from exc

    @staticmethod
    def parse_index(raw: str, count: int) -> int | None:
        """Parse a 1-based menu index and return a 0-based index."""
        raw = raw.strip().lstrip("#")
        if not raw.isdigit():
            return None
        index = int(raw)
        if 1 <= index <= count:
            return index - 1
        return None

    @staticmethod
    def format_size(size: int) -> str:
        value = float(size)
        for unit in ("B", "KB", "MB", "GB"):
            if value < 1024 or unit == "GB":
                return f"{value:.0f} {unit}" if unit == "B" else f"{value:.1f} {unit}"
            value /= 1024
        return f"{value:.1f} GB"
