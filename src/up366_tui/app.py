"""Interactive application state and menu flows."""
from __future__ import annotations

import json
import os
import traceback
from pathlib import Path
from typing import Any

from up366 import Job, Up366
from up366.errors import Up366APIError, Up366Error

from .jobstatus import (
    STATE_FINISHED,
    STATE_UNFINISHED,
    JobState,
    build_status_indexes,
    detail_state,
    resolve_job_state,
)
from .paper import Paper, parse_paper_xml, question_type_name
from .readaloud import build_manual_plan, build_plan, save_plan, submit_plan
from .ui import UI, UserExit
from .wordpk import bucket_id, select_word_pk_words, selection_rows, usable_words


class App:
    """One logged-in session and all menu flows."""

    MENU = [
        ("1", "选择课程 / 作业"),
        ("2", "单词 PK 提交"),
        ("3", "跟读作业（收集 mp3；提交留空）"),
        ("4", "作业详情 / 试卷结构"),
        ("5", "错题本概览（只读）"),
        ("6", "普通作业提交（留空）"),
        ("7", "帮助 / 使用说明"),
    ]

    def __init__(self, session: Up366, ui: UI):
        self.session = session
        self.ui = ui
        self.courses: list[dict[str, Any]] | None = None
        self.course: dict[str, Any] | None = None
        self.jobs: list[Job] = []
        self.jobs_course_id: Any = None
        self.job: Job | None = None
        self.job_detail: dict[str, Any] | None = None
        self.papers: dict[str, Paper | None] = {}
        self.buckets: dict[str, list[dict[str, Any]]] = {}
        self.finished_by_task: dict[str, dict[str, Any]] = {}
        self.returned_by_task: dict[str, dict[str, Any]] = {}
        self.status_course_id: Any = None
        self.detail_status: dict[str, str] = {}

    # --- lifecycle -------------------------------------------------

    def run(self) -> None:
        while True:
            try:
                self._render_main()
                choice = self.ui.ask("请选择")
                if choice == "":
                    continue
                if choice in ("0", "q", "quit", "exit"):
                    self.ui.info("再见。")
                    return
                if choice in ("h", "help"):
                    self.help_flow()
                    continue
                handler = self._menu_handlers().get(choice)
                if handler is None:
                    self.ui.error("无效选择。")
                    self.ui.pause()
                    continue
                handler()
            except UserExit:
                raise
            except KeyboardInterrupt:
                self.ui.line("")
                self.ui.info("已退出。")
                return
            except Exception as exc:
                self._show_error(exc)
                self.ui.pause()

    def _menu_handlers(self) -> dict[str, Any]:
        return {
            "1": self.select_course_and_job,
            "2": self.word_pk_flow,
            "3": self.readaloud_flow,
            "4": self.job_detail_flow,
            "5": self.wrongnote_flow,
            "6": self.ordinary_homework_stub,
            "7": self.help_flow,
        }

    def _show_error(self, exc: Exception) -> None:
        if isinstance(exc, Up366APIError):
            if exc.code == -13:
                self.ui.error("服务器返回 -13：重复作答或次数用尽。")
            elif exc.code in (-21, -22):
                self.ui.error("登录态失效（-21/-22）；请重新启动程序登录。")
            else:
                self.ui.error(f"接口错误（code={exc.code}）：{exc.message}")
            return
        if isinstance(exc, Up366Error):
            self.ui.error(str(exc))
            return
        self.ui.error(f"{type(exc).__name__}: {exc}")
        if self.ui.verbose:
            traceback.print_exc()

    # --- rendering -------------------------------------------------

    def _render_main(self) -> None:
        self.ui.clear()
        self.ui.rule("=")
        self.ui.line(
            self.ui.style(" up366 TUI", self.ui.BOLD + self.ui.CYAN)
            + f"  账号: {self.session.username}  uType: {self.session.user_type}"
        )
        self.ui.line(f" 课程: {self._course_label()}")
        self.ui.line(f" 作业: {self._job_label()}")
        self.ui.rule("=")
        for key, label in self.MENU:
            self.ui.line(f"  {key}) {label}")
        self.ui.line("  0) 退出")
        self.ui.line("")

    def _course_label(self) -> str:
        if self.course is None:
            return "(未选择)"
        return (
            f"{self.course.get('courseName') or '(未命名)'} "
            f"[{self.course.get('courseId')}]"
        )

    def _job_label(self) -> str:
        if self.job is None:
            return "(未选择)"
        tags = " ".join(self._job_tags(self.job))
        return f"{self.job.name} [{self.job.job_id}] {tags}"

    def _job_tags(self, job: Job) -> list[str]:
        tags: list[str] = []
        if job.is_word_pk:
            tags.append("[单词PK]")
        elif any(keyword in job.name for keyword in ("跟读", "朗读")):
            tags.append("[跟读?]")
        tags.append(f"[{self._job_state(job).label}]")
        return tags

    # --- course / job state ---------------------------------------

    def _course_id(self) -> Any:
        return self.course.get("courseId") if self.course is not None else None

    def _ensure_course(self) -> dict[str, Any] | None:
        if self.course is None:
            self.select_course_and_job()
        return self.course

    def _ensure_job(self) -> Job | None:
        if self._ensure_course() is None:
            return None
        if self.job is None:
            self.select_job()
        return self.job

    def select_course_and_job(self) -> None:
        while True:
            if self.courses is None:
                self.ui.info("正在读取课程列表…")
                self.courses = self.session.courses()
            if not self.courses:
                self.ui.warn("当前账号没有课程。")
                self.ui.pause()
                return
            self.ui.clear()
            self.ui.title("课程列表")
            for index, course in enumerate(self.courses, 1):
                self.ui.line(
                    f"  {index:>2}) {course.get('courseName') or '(未命名)'}  "
                    f"[{course.get('courseId')}]"
                )
            self.ui.line("   r) 刷新")
            self.ui.line("   0) 返回")
            raw = self.ui.ask("选择课程")
            if raw in ("", "0"):
                return
            if raw.lower() == "r":
                self.courses = None
                continue
            index = self.ui.parse_index(raw, len(self.courses))
            if index is None:
                self.ui.error("无效选择。")
                self.ui.pause()
                continue
            self.course = self.courses[index]
            self.jobs = []
            self.jobs_course_id = None
            self.job = None
            self.job_detail = None
            self.status_course_id = None
            self.finished_by_task = {}
            self.returned_by_task = {}
            self.detail_status = {}
            self.select_job()
            return

    def select_job(self) -> None:
        course_id = self._course_id()
        if course_id is None:
            self.ui.error("请先选择课程。")
            self.ui.pause()
            return
        while True:
            jobs = self._ensure_jobs(course_id)
            if not jobs:
                self.ui.warn("该课程没有作业。")
                self.ui.pause()
                return
            self.ui.clear()
            self.ui.title("作业列表")
            for index, job in enumerate(jobs, 1):
                tags = " ".join(self._job_tags(job))
                self.ui.line(f"  {index:>2}) {job.name}  {tags}")
            self.ui.line("   r) 刷新")
            self.ui.line("   0) 返回")
            raw = self.ui.ask("选择作业")
            if raw in ("", "0"):
                return
            if raw.lower() == "r":
                self.jobs = []
                self.jobs_course_id = None
                self.status_course_id = None
                continue
            index = self.ui.parse_index(raw, len(jobs))
            if index is None:
                self.ui.error("无效选择。")
                self.ui.pause()
                continue
            self._set_job(jobs[index])
            return

    def _set_job(self, job: Job) -> None:
        self.job = job
        self.job_detail = None
        self.ui.clear()
        self.ui.title("作业摘要")
        self.ui.line(f"名称: {job.name}")
        self.ui.line(f"jobId: {job.job_id}")
        self.ui.line(f"contentId: {job.content_id}")
        self.ui.line(f"bookId: {job.book_id}")
        self.ui.line(f"courseId: {job.course_id}")
        detail = self._fetch_job_detail(job)
        state = self._job_state(job)
        self.ui.line(f"状态: {state.label}（来源: {state.source}）")
        if detail:
            self.ui.line(
                f"taskStatus: {detail.get('taskStatus')}  "
                f"stuStatus: {detail.get('stuStatus')}  "
                f"paperId: {detail.get('paperId')}"
            )
        self.ui.pause()

    def _job_info_args(self, job: Job) -> tuple[Any, Any, Any, Any] | None:
        course_id = job.course_id if job.course_id is not None else self._course_id()
        if (
            course_id is None
            or not job.job_id
            or not job.content_id
            or not job.book_id
        ):
            return None
        return (course_id, job.job_id, job.content_id, job.book_id)

    def _fetch_job_detail(
        self, job: Job, *, force: bool = False
    ) -> dict[str, Any] | None:
        if self.job is job and self.job_detail is not None and not force:
            return self.job_detail
        args = self._job_info_args(job)
        if args is None:
            self.ui.warn("该作业缺少 courseId/contentId/bookId，无法查询详情。")
            return None
        try:
            detail = self.session.job_info(*args)
        except Up366Error as exc:
            self.ui.warn(f"作业详情读取失败：{exc}")
            return None
        if job.job_id:
            state = detail_state(detail)
            if state is not None:
                self.detail_status[str(job.job_id)] = state
        if self.job is job:
            self.job_detail = detail
        return detail

    def _load_course_status(self, course_id: Any, *, force: bool = False) -> None:
        if not force and self.status_course_id == course_id:
            return
        self.status_course_id = course_id
        try:
            data = self.session.course_finished_jobs(course_id)
        except Up366Error as exc:
            self.finished_by_task = {}
            self.returned_by_task = {}
            self.ui.warn(f"完成状态读取失败（仅使用作业字段）：{exc}")
            return
        self.finished_by_task, self.returned_by_task = build_status_indexes(data)

    def _ensure_jobs(self, course_id: Any, *, force: bool = False) -> list[Job]:
        if force or self.jobs_course_id != course_id:
            self.ui.info("正在读取作业列表…")
            self.jobs = self.session.jobs(course_id)
            self.jobs_course_id = course_id
            if force:
                self.detail_status.clear()
            self._load_course_status(course_id, force=True)
        return self.jobs

    def _job_state(self, job: Job) -> JobState:
        key = str(job.job_id) if job.job_id else ""
        return resolve_job_state(
            job,
            finished=self.finished_by_task,
            returned=self.returned_by_task,
            detail_status=self.detail_status.get(key) if key else None,
        )

    def _jobs_for_current_course(self) -> list[Job]:
        course_id = self._course_id()
        if course_id is None:
            return []
        return self._ensure_jobs(course_id)

    def _choose_job_from(self, candidates: list[Job], title: str) -> Job | None:
        while True:
            self.ui.clear()
            self.ui.title(title)
            for index, job in enumerate(candidates, 1):
                tags = " ".join(self._job_tags(job))
                self.ui.line(f"  {index:>2}) {job.name}  {tags}")
            self.ui.line("   0) 返回")
            raw = self.ui.ask("选择作业")
            if raw in ("", "0"):
                return None
            index = self.ui.parse_index(raw, len(candidates))
            if index is None:
                self.ui.error("无效选择。")
                self.ui.pause()
                continue
            self.job = candidates[index]
            self.job_detail = None
            return self.job

    # --- word PK ---------------------------------------------------

    def _ensure_job_for_word_pk(self) -> Job | None:
        if self._ensure_course() is None:
            return None
        if self.job is not None and self.job.is_word_pk:
            return self.job
        candidates = [
            job for job in self._jobs_for_current_course() if job.is_word_pk
        ]
        if not candidates:
            self.ui.warn("当前课程没有单词 PK 作业。")
            self.ui.pause()
            return None
        return self._choose_job_from(candidates, "选择单词 PK 作业")

    def _bucket_words(self, bucket: str) -> list[dict[str, Any]]:
        if bucket not in self.buckets:
            self.ui.info(f"正在读取词库（bucketId={bucket}）…")
            self.buckets[bucket] = self.session.bucket_words(bucket)
        return self.buckets[bucket]

    def word_pk_flow(self) -> None:
        job = self._ensure_job_for_word_pk()
        if job is None:
            return
        bucket = bucket_id(job)
        if bucket is None:
            self.ui.error("该作业的 xotParam 中没有 pkBucketId，无法获取词库。")
            self.ui.pause()
            return
        pool = usable_words(self._bucket_words(bucket))
        if not pool:
            self.ui.error("词库为空或缺少 entryId/pkid。")
            self.ui.pause()
            return

        self.ui.clear()
        self.ui.title("单词 PK")
        self.ui.line(f"作业: {job.name}")
        self.ui.line(f"词库可用单词: {len(pool)}")
        state = self._job_state(job)
        if state.is_finished:
            self.ui.warn(
                f"该作业显示为「{state.label}」；重复提交预计返回 -13。"
            )
            if not self.ui.ask_yes_no("仍然继续？", False):
                return

        while True:
            score = self.ui.ask_int("分数（答对词数）", minimum=0)
            total = self.ui.ask_int(
                "total（本局总词数）", minimum=1, maximum=len(pool)
            )
            if score > total:
                self.ui.error("分数不能大于 total，请重新输入。")
                continue
            break

        selection = select_word_pk_words(pool, score=score, total=total)
        self._show_word_pk_preview(selection)
        role = self.ui.ask("角色名（Red/Blue）", default="Red")
        if role.lower() in ("red", "blue"):
            role = role.capitalize()
        else:
            self.ui.warn(f"角色名 {role!r} 不是 Red/Blue；服务器可能拒绝。")
        if not self.ui.ask_yes_no("确认提交到服务器？", False):
            self.ui.info("已取消，未提交。")
            return

        self.ui.info("正在提交…")
        try:
            result = self.session.submit_word_pk(
                job,
                score,
                list(selection.correct),
                list(selection.wrong),
                role_name=role,
            )
        except Up366APIError as exc:
            if exc.code == -13:
                self.ui.error("服务器返回 -13：重复作答或次数用尽，未提交成功。")
                self.ui.pause()
                return
            raise
        self.ui.ok("提交成功。")
        if result:
            self.ui.line(json.dumps(result, ensure_ascii=False, indent=2)[:3000])
        if job.job_id:
            self.detail_status[str(job.job_id)] = STATE_FINISHED
        self._refresh_job_status(job, allow_unfinished=False)
        self.ui.pause()

    def _show_word_pk_preview(self, selection: Any) -> None:
        self.ui.clear()
        self.ui.title("单词 PK 预览")
        self.ui.line(
            f"共 {selection.total} 词：正确 {selection.right_count}，"
            f"错误 {selection.wrong_count}"
        )
        rows = selection_rows(selection, limit=50)
        self.ui.table(
            ["#", "单词", "释义", "结果"],
            rows,
            aligns=["right", "left", "left", "left"],
        )
        if selection.total > 50:
            self.ui.line(f"… 其余 {selection.total - 50} 词未展开")

    def _refresh_job_status(
        self, job: Job, *, allow_unfinished: bool = True
    ) -> None:
        args = self._job_info_args(job)
        if args is None:
            return
        try:
            status = self.session.job_status(*args)
        except Up366Error as exc:
            self.ui.warn(f"作业状态刷新失败：{exc}")
            return
        key = str(job.job_id) if job.job_id else ""
        if key:
            if status.is_finished:
                self.detail_status[key] = STATE_FINISHED
            elif allow_unfinished:
                self.detail_status[key] = STATE_UNFINISHED
            else:
                self.ui.warn("服务器暂未反映完成状态；已按提交成功标记为已交。")
        self.ui.line(
            f"作业状态：taskStatus={status.task_status}  "
            f"stuStatus={status.stu_status}  "
            f"submitTime={status.submit_time}  "
            f"已完成={status.is_finished}"
        )

    # --- read-aloud ------------------------------------------------

    def _ensure_job_for_readaloud(self) -> Job | None:
        if self._ensure_course() is None:
            return None
        if self.job is not None and not self.job.is_word_pk:
            return self.job
        candidates = [
            job for job in self._jobs_for_current_course() if not job.is_word_pk
        ]
        if not candidates:
            self.ui.warn("当前课程没有可选的普通/跟读作业。")
            self.ui.pause()
            return None
        return self._choose_job_from(candidates, "选择跟读作业（读取试卷后确认）")

    def _paper(self, paper_id: str) -> Paper | None:
        if paper_id in self.papers:
            return self.papers[paper_id]
        self.ui.info(f"正在读取试卷结构（paperId={paper_id}）…")
        try:
            xml = self.session.paper_detail(paper_id)
        except Up366Error as exc:
            self.ui.warn(f"试卷结构读取失败：{exc}")
            self.papers[paper_id] = None
            return None
        if not xml:
            self.ui.warn("试卷结构为空。")
            self.papers[paper_id] = None
            return None
        try:
            paper = parse_paper_xml(xml if isinstance(xml, str) else str(xml))
        except ValueError as exc:
            self.ui.warn(str(exc))
            self.papers[paper_id] = None
            return None
        self.papers[paper_id] = paper
        return paper

    def _show_question_type_counts(self, paper: Paper) -> None:
        if not paper.questions:
            self.ui.line("试卷题目: (无)")
            return
        self.ui.line("题目类型统计:")
        for question_type, count in paper.question_type_counts.items():
            self.ui.line(
                f"  - {question_type_name(question_type)}"
                f"（question_type={question_type}）: {count}"
            )

    def _show_readaloud_questions(self, job: Job, questions: list[Any]) -> None:
        self.ui.clear()
        self.ui.title("跟读作业")
        self.ui.line(f"作业: {job.name}")
        self.ui.line(
            f"跟读题数: {len(questions)}  →  需要 {len(questions)} 段 mp3"
            "（每题一段）"
        )
        rows = []
        for index, question in enumerate(questions, 1):
            short_id = question.element_id[:12] + (
                "…" if len(question.element_id) > 12 else ""
            )
            rows.append(
                (
                    str(index),
                    short_id,
                    "-" if question.score is None else f"{question.score:g}",
                    "-" if question.duration is None else f"{question.duration}s",
                    str(question.sentence_count),
                    question.preview[:80],
                )
            )
        self.ui.table(
            ["#", "elementId", "分值", "时长", "句数", "内容预览"],
            rows,
            aligns=["right", "left", "right", "right", "right", "left"],
        )
        self.ui.line("")
        self.ui.warn("顺序必须与上表一致：第 1 段对应第 1 题，以此类推。")

    def _collect_mp3_paths(
        self, count: int, questions: list[Any]
    ) -> list[Path] | None:
        paths: list[Path] = []
        for index in range(1, count + 1):
            question = questions[index - 1] if index - 1 < len(questions) else None
            label = (
                f"第 {index}/{count} 题"
                if question is not None
                else f"第 {index}/{count} 段"
            )
            if question is not None and question.preview:
                self.ui.line(f"{label}: {question.preview[:60]}")
            while True:
                raw = self.ui.ask(f"{label} mp3 路径（q 取消）")
                if raw.lower() in ("q", "quit", "exit"):
                    self.ui.info("已取消。")
                    return None
                cleaned = raw.strip().strip('"').strip("'").strip()
                if not cleaned:
                    self.ui.error("路径不能为空。")
                    continue
                path = Path(
                    os.path.expandvars(os.path.expanduser(cleaned))
                )
                if not path.exists():
                    self.ui.error(f"文件不存在：{path}")
                    continue
                if not path.is_file():
                    self.ui.error(f"不是普通文件：{path}")
                    continue
                if path.suffix.lower() != ".mp3":
                    self.ui.error(
                        f"需要 .mp3 文件（当前后缀 {path.suffix or '(无)'}）。"
                    )
                    continue
                paths.append(path)
                self.ui.ok(
                    f"已记录：{path.name}（{self.ui.format_size(path.stat().st_size)}）"
                )
                break

        counts: dict[str, int] = {}
        for path in paths:
            key = str(path.resolve())
            counts[key] = counts.get(key, 0) + 1
        duplicates = [key for key, count in counts.items() if count > 1]
        if duplicates:
            self.ui.warn("检测到重复文件：" + ", ".join(duplicates))
            if not self.ui.ask_yes_no("仍然继续？", False):
                return None
        return paths

    def _show_mp3_summary(self, plan: Any) -> None:
        self.ui.clear()
        self.ui.title("跟读计划")
        for question, file_info in zip(plan.questions, plan.mp3_files):
            self.ui.line(
                f"  Q{question['index']}: {file_info['path']}  "
                f"({self.ui.format_size(file_info['size'])})"
            )
        self.ui.line(f"共 {len(plan.mp3_files)} 段。")

    def readaloud_flow(self) -> None:
        job = self._ensure_job_for_readaloud()
        if job is None:
            return
        detail = self._fetch_job_detail(job)
        paper_id = detail.get("paperId") if detail else None
        if not paper_id:
            paper_id = job.paper_id
        paper = self._paper(str(paper_id)) if paper_id else None

        if paper is not None:
            questions = paper.readaloud_questions
            if not questions:
                self.ui.clear()
                self.ui.title("跟读作业检查")
                self.ui.line(f"作业: {job.name}")
                self.ui.line(f"paperId: {paper_id}")
                self._show_question_type_counts(paper)
                self.ui.warn("该试卷没有跟读题（question_type=9），不是跟读作业。")
                self.ui.pause()
                return
            self._show_readaloud_questions(job, questions)
            paths = self._collect_mp3_paths(len(questions), questions)
            if paths is None:
                return
            plan = build_plan(
                job=job,
                paper=paper,
                paths=paths,
                course_id=self._course_id(),
                paper_id=paper_id,
            )
        else:
            self.ui.clear()
            self.ui.title("跟读作业检查")
            self.ui.line(f"作业: {job.name}")
            self.ui.line(f"paperId: {paper_id or '(无)'}")
            self.ui.warn("无法读取试卷结构（详情缺失或接口失败）。")
            if not self.ui.ask_yes_no("是否手动输入需要的 mp3 段数？", True):
                return
            count = self.ui.ask_int("需要多少段 mp3？", minimum=1)
            paths = self._collect_mp3_paths(count, [])
            if paths is None:
                return
            plan = build_manual_plan(
                job,
                paths,
                course_id=self._course_id(),
                paper_id=paper_id or "",
            )

        self._show_mp3_summary(plan)
        path = save_plan(plan)
        self.ui.ok(f"计划已保存：{path}")
        try:
            submit_plan(self.session, plan)
        except NotImplementedError as exc:
            self.ui.warn(str(exc))
        self.ui.info("本次只写入了本地计划文件；未上传音频，也未改动服务器数据。")
        self.ui.pause()

    # --- detail / wrongnote / stubs -------------------------------

    def job_detail_flow(self) -> None:
        job = self._ensure_job()
        if job is None:
            return
        self.ui.clear()
        self.ui.title("作业详情")
        self.ui.line(f"名称: {job.name}")
        self.ui.line(f"jobId: {job.job_id}")
        self.ui.line(
            f"courseId: {job.course_id}  contentId: {job.content_id}  "
            f"bookId: {job.book_id}"
        )
        detail = self._fetch_job_detail(job, force=True)
        state = self._job_state(job)
        self.ui.line(f"状态: {state.label}（来源: {state.source}）")
        if detail:
            self.ui.line("")
            self.ui.line("job/info:")
            self.ui.line(json.dumps(detail, ensure_ascii=False, indent=2))
        paper_id = detail.get("paperId") if detail else job.paper_id
        if paper_id:
            paper = self._paper(str(paper_id))
            if paper is not None:
                self.ui.line("")
                self._show_question_type_counts(paper)
                questions = paper.readaloud_questions
                if questions:
                    self.ui.line(f"跟读题: {len(questions)} 段 mp3")
                    for index, question in enumerate(questions, 1):
                        self.ui.line(
                            f"  {index}) elementId={question.element_id} "
                            f"分值={question.score} 时长={question.duration} "
                            f"句数={question.sentence_count}"
                        )
                        if question.preview:
                            self.ui.line(f"     {question.preview[:100]}")
        self.ui.pause()

    def wrongnote_flow(self) -> None:
        self.ui.clear()
        self.ui.title("错题本（只读）")
        self.ui.info("正在读取学科…")
        subjects = self.session.wrong_subjects()
        if not subjects:
            self.ui.info("没有错题本数据。")
            self.ui.pause()
            return
        rows = [
            (
                str(index),
                str(subject.subject_name or ""),
                str(subject.wrong_count or 0),
                str(subject.killed_count or 0),
                str(subject.killed_count_lately or 0),
            )
            for index, subject in enumerate(subjects, 1)
        ]
        self.ui.table(
            ["#", "学科", "错题", "已消灭", "近期消灭"],
            rows,
            aligns=["right", "left", "right", "right", "right"],
        )
        raw = self.ui.ask("选择学科编号（0 返回）")
        if raw in ("", "0"):
            return
        index = self.ui.parse_index(raw, len(subjects))
        if index is None:
            self.ui.error("无效选择。")
            self.ui.pause()
            return
        subject = subjects[index]
        stage_id = subject.stage_id
        subject_id = subject.subject_id
        self.ui.info("正在读取统计…")
        stat = self.session.wrong_subject_stat(
            book_id="0", stage_id=stage_id, subject_id=subject_id
        )
        self.ui.line(
            f"错题 {stat.wrong_count} / 已消灭 {stat.killed_count} / "
            f"近期消灭 {stat.killed_count_lately}"
        )
        books = self.session.wrong_books(
            stage_id=stage_id, subject_id=subject_id
        )
        self.ui.line("")
        self.ui.table(
            ["bookId", "书名"],
            [(str(book.book_id), str(book.book_name)) for book in books],
        )
        courses = self.session.wrong_courses(
            stage_id=stage_id, subject_id=subject_id
        )
        self.ui.line("")
        self.ui.table(
            ["courseId", "课程"],
            [(str(course.course_id), str(course.course_name)) for course in courses],
        )
        self.ui.pause()

    def ordinary_homework_stub(self) -> None:
        self.ui.clear()
        self.ui.title("普通作业提交")
        self.ui.warn("按当前需求，普通作业提交功能留空，未实现。")
        self.ui.line("已实现：单词 PK 提交、跟读作业 mp3 收集与本地计划。")
        self.ui.pause()

    def help_flow(self) -> None:
        self.ui.clear()
        self.ui.title("帮助")
        self.ui.line("环境变量：")
        self.ui.line("  UP366_USER       账号")
        self.ui.line("  UP366_PASS       口令（不写入磁盘）")
        self.ui.line("  UP366_CLIENT_ID  覆盖 clientId（可选）")
        self.ui.line("")
        self.ui.line("流程：")
        self.ui.line("  1) 选择课程 / 作业：浏览课程与作业，作业列表标记 PK / 已交。")
        self.ui.line("  2) 单词 PK 提交：输入分数与 total；程序从词库随机抽词，")
        self.ui.line("     分数 = 正确词数，其余为错误词；确认后真实提交。")
        self.ui.line("  3) 跟读作业：读取试卷结构，统计跟读题（每题一段 mp3），")
        self.ui.line("     逐段输入本地 mp3 路径；仅保存本地计划，提交接口留空。")
        self.ui.line("  4) 作业详情：查看 job/info 原始 JSON 与试卷题型统计。")
        self.ui.line("  5) 错题本：只读浏览学科、统计、书籍与课程。")
        self.ui.line("  6) 普通作业提交：按需求留空。")
        self.ui.line("")
        self.ui.warn("单词 PK 是真实的写操作；同一作业重复提交通常返回 -13。")
        self.ui.pause()
