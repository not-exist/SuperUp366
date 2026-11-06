"""Command-line entry point for the up366 TUI."""
from __future__ import annotations

import argparse
import getpass
import os
import sys
import traceback
import urllib.parse
from random import Random

from up366 import Up366, login
from up366.errors import Up366AuthError, Up366Error, Up366RequestError

from . import __version__
from .app import App
from .paper import parse_paper_xml
from .ui import UI, UserExit
from .wordpk import select_word_pk_words

_ATTACHMENT_XML = (
    "<root><paragraph>"
    '<sentences id="1"><content><![CDATA[Hello world.]]></content>'
    "<content_cn><![CDATA[你好世界。]]></content_cn>"
    '<netfiles><item netfile="net/0001.net"/></netfiles></sentences>'
    "</paragraph></root>"
)
SAMPLE_PAPER = (
    '<?xml version="1.0" encoding="UTF-8"?>\n'
    "<elements>\n"
    '  <element id="Q1" type="3">\n'
    "    <question_no>1</question_no>\n"
    "    <question_text><![CDATA[<img src=\"media.jpg\">Hello world.]]></question_text>\n"
    "    <question_score>10.0</question_score>\n"
    "    <question_type>9</question_type>\n"
    "    <duration>82</duration>\n"
    "    <attachment><![CDATA["
    + urllib.parse.quote_plus(_ATTACHMENT_XML)
    + "]]></attachment>\n"
    "  </element>\n"
    '  <element id="Q2" type="2">\n'
    "    <question_no>2</question_no>\n"
    "    <question_score>5</question_score>\n"
    "    <question_type>4</question_type>\n"
    "    <duration>30</duration>\n"
    "  </element>\n"
    "</elements>\n"
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="up366-tui",
        description="up366（天学网）命令行 TUI：课程/作业浏览、单词 PK 提交、跟读 mp3 收集。",
    )
    parser.add_argument(
        "--username",
        help="账号；缺省读 UP366_USER，仍为空则交互输入",
    )
    parser.add_argument(
        "--password",
        help="口令；缺省读 UP366_PASS，仍为空则交互输入（命令行参数会留在历史记录里）",
    )
    parser.add_argument(
        "--client-id",
        help="覆盖 clientId；缺省读 UP366_CLIENT_ID 或本机指纹",
    )
    parser.add_argument("--no-color", action="store_true", help="禁用 ANSI 颜色")
    parser.add_argument(
        "--no-clear", action="store_true", help="不清屏（保留终端滚动历史）"
    )
    parser.add_argument("--verbose", action="store_true", help="打印异常堆栈")
    parser.add_argument(
        "--self-check",
        action="store_true",
        help="离线自检试卷解析与随机选词，随后退出",
    )
    parser.add_argument(
        "--version", action="version", version=f"%(prog)s {__version__}"
    )
    return parser


def run_self_check(ui: UI) -> int:
    try:
        paper = parse_paper_xml(SAMPLE_PAPER)
        assert len(paper.questions) == 2, paper.questions
        assert len(paper.readaloud_questions) == 1, paper.readaloud_questions
        question = paper.readaloud_questions[0]
        assert question.score == 10.0
        assert question.duration == 82
        assert question.sentences == ("Hello world.",)
        assert question.preview == "Hello world."
        assert paper.question_type_counts == {4: 1, 9: 1}

        words = [
            {"entryId": index, "entry": f"word{index}", "paraphrase": f"释义{index}"}
            for index in range(1, 11)
        ]
        selection = select_word_pk_words(
            words, score=7, total=10, rng=Random(1)
        )
        assert selection.right_count == 7
        assert selection.wrong_count == 3
        assert len({word["entryId"] for word in selection.correct}) == 7
    except Exception as exc:
        ui.error(f"self-check 失败：{exc}")
        if ui.verbose:
            traceback.print_exc()
        return 1
    ui.ok("self-check OK")
    return 0


def _login_loop(ui: UI, args: argparse.Namespace) -> Up366 | None:
    explicit = bool(args.username and args.password)
    username = args.username or os.environ.get("UP366_USER")
    password = args.password or os.environ.get("UP366_PASS")
    for _ in range(3):
        if not username:
            username = ui.ask("账号")
        if not password:
            try:
                password = getpass.getpass("口令: ")
            except EOFError as exc:
                raise UserExit() from exc
        if not username or not password:
            ui.error("账号和口令不能为空。")
            if explicit:
                return None
            username = os.environ.get("UP366_USER")
            password = os.environ.get("UP366_PASS")
            continue

        ui.info(f"正在登录 {username} …")
        try:
            session = login(username, password)
        except Up366AuthError as exc:
            ui.error(str(exc))
            if explicit:
                return None
            if not ui.ask_yes_no("重新输入账号口令？", True):
                return None
            username = os.environ.get("UP366_USER")
            password = os.environ.get("UP366_PASS")
            continue
        except Up366RequestError as exc:
            ui.error(f"网络错误：{exc}")
            if not ui.ask_yes_no("重试登录？", True):
                return None
            continue
        except Up366Error as exc:
            ui.error(str(exc))
            return None
        ui.ok(f"登录成功：{username}（uType={session.user_type}）")
        return session
    ui.error("登录失败次数过多。")
    return None


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    ui = UI(
        color=False if args.no_color else None,
        verbose=args.verbose,
        clear_screen=not args.no_clear,
    )
    if args.self_check:
        return run_self_check(ui)
    if args.client_id:
        os.environ["UP366_CLIENT_ID"] = args.client_id

    try:
        session = _login_loop(ui, args)
    except UserExit:
        ui.info("已取消。")
        return 1
    if session is None:
        return 2

    try:
        App(session, ui).run()
    except UserExit:
        ui.line("")
        ui.info("已退出。")
    except KeyboardInterrupt:
        ui.line("")
        ui.info("已退出。")
    except Exception as exc:
        ui.error(f"未处理异常：{exc}")
        if args.verbose:
            traceback.print_exc()
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
