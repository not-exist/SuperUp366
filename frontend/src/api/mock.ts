// Browser preview backend.
//
// The UI is developed and reviewed in a plain browser as well as inside Tauri.
// When no Tauri runtime is present, `invoke` routes every command here so the
// pages render with realistic data instead of a white screen. The data is
// deterministic apart from short simulated network delays.

import type {
  Account,
  AnswerDocument,
  AnswerQuestion,
  Book,
  BookDetail,
  BookJob,
  CommandMap,
  CorrectionDraw,
  CorrectionOverview,
  CorrectionQuestion,
  CorrectionStatus,
  CorrectionSubmit,
  Course,
  JobRow,
  JobStatus,
  JobStatusKind,
  ReadAloudPreview,
  ReadAloudTask,
  ReadAloudUpload,
  Settings,
  WordPkPreview,
  WordPkResult,
} from "./types";

const DAY = 86_400_000;
const BASE = Date.parse("2026-11-07T09:00:00.000Z");

function iso(dayOffset: number, hour = 20): string {
  return new Date(BASE + dayOffset * DAY + (hour - 9) * 3_600_000).toISOString();
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function latency(): Promise<void> {
  return sleep(120 + Math.floor(Math.random() * 160));
}

function hash(value: string): number {
  let h = 2166136261;
  for (let i = 0; i < value.length; i += 1) {
    h ^= value.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h);
}

// --- accounts ---------------------------------------------------------

let accounts: Account[] = [
  {
    account_id: "acc-1",
    username: "student_a",
    display_name: "陈同学",
    school_name: "天学网实验中学",
    active: true,
    session_valid: true,
    expires_at: iso(30),
  },
  {
    account_id: "acc-2",
    username: "student_b",
    display_name: "林同学",
    school_name: "天学网实验中学",
    active: false,
    session_valid: true,
    expires_at: iso(12),
  },
];

// --- courses and jobs -------------------------------------------------

const courseSeeds: Course[] = [
  {
    course_id: "course-1",
    name: "高二英语 · 1 班",
    class_name: "高二(1)班",
    teacher_name: "李老师",
  },
  {
    course_id: "course-2",
    name: "高二英语 · 3 班",
    class_name: "高二(3)班",
    teacher_name: "周老师",
  },
  {
    course_id: "course-3",
    name: "英语培优 · 周末",
    class_name: "培优班",
    teacher_name: "王老师",
  },
];

const jobTitles = [
  "Unit 1 词汇巩固",
  "Unit 2 阅读理解",
  "听力专项训练",
  "语法填空练习",
  "单元综合测试",
  "完形填空强化",
  "单词卡 · Unit 3",
  "写作训练：My Weekend",
  "课文跟读 Unit 4",
  "期末模拟卷",
  "短语搭配练习",
  "阅读七选五",
  "短文改错",
  "单词卡 · Unit 5",
];

const statusRotation: JobStatusKind[] = [
  "finished",
  "pending",
  "finished",
  "returned",
  "expired",
  "pending",
  "finished",
  "unknown",
  "pending",
  "finished",
  "returned",
  "pending",
  "expired",
  "finished",
];

const scores = [92, 88, 76, 100, 65];

function makeStatus(kind: JobStatusKind, seed: number): JobStatus {
  if (kind === "finished") {
    return { kind, score: scores[seed % scores.length] };
  }
  return { kind };
}

function buildJobs(): JobRow[] {
  return courseSeeds.flatMap((course, courseIndex) =>
    jobTitles.map((name, jobIndex) => {
      const seed = courseIndex * 7 + jobIndex;
      const statusKind = statusRotation[seed % statusRotation.length];
      const deadline =
        statusKind === "expired" || statusKind === "unknown"
          ? iso(-(3 + (seed % 5)))
          : statusKind === "finished"
            ? iso(-(1 + (seed % 4)))
            : iso(1 + (seed % 9));
      const kind = name.includes("单词卡") ? "word_pk" : "ordinary";
      const status = makeStatus(statusKind, seed);
      return {
        job_id: `job-${courseIndex + 1}-${jobIndex + 1}`,
        course_id: course.course_id,
        course_name: course.name,
        class_name: course.class_name,
        name,
        kind,
        status,
        score: status.score ?? null,
        deadline,
        read_aloud: name.includes("跟读"),
      } satisfies JobRow;
    }),
  );
}

const jobs = buildJobs();

// --- books ------------------------------------------------------------

const bookSeeds: Book[] = [
  {
    book_id: "book-1",
    name: "英语必修 第一册",
    subject: "英语",
    cover_url: null,
    is_2025_reader: false,
  },
  {
    book_id: "book-2",
    name: "英语必修 第二册",
    subject: "英语",
    cover_url: null,
    is_2025_reader: false,
  },
  {
    book_id: "book-3",
    name: "高中英语阅读进阶",
    subject: "英语",
    cover_url: null,
    is_2025_reader: true,
  },
];

const bookChapterTitles = [
  "Unit 1 Teenage Life",
  "Unit 2 Travelling Around",
  "Unit 3 Sports and Fitness",
];

function bookJob(bookId: string, chapterIndex: number, jobIndex: number): BookJob {
  const seed = hash(`${bookId}-${chapterIndex}-${jobIndex}`);
  const statusKind = statusRotation[seed % statusRotation.length];
  const status = makeStatus(statusKind, seed);
  return {
    job_id: `${bookId}-c${chapterIndex + 1}-j${jobIndex + 1}`,
    name: ["课文精读", "词汇跟读", "单元练习", "单词卡 · 复习"][jobIndex % 4],
    kind: jobIndex === 3 ? "word_pk" : "ordinary",
    status,
    score: status.score ?? null,
    deadline: iso(statusKind === "finished" ? -2 : 3 + (seed % 6)),
    read_aloud: jobIndex === 1,
  };
}

const books: BookDetail[] = bookSeeds.map((book, bookIndex) => {
  const chapters = bookChapterTitles.map((title, chapterIndex) => ({
    chapter_id: `${book.book_id}-ch-${chapterIndex + 1}`,
    title,
    jobs: Array.from({ length: 4 }, (_, jobIndex) =>
      bookJob(book.book_id, chapterIndex, jobIndex),
    ),
  }));
  const jobs = chapters.flatMap((chapter) => chapter.jobs);
  return {
    ...book,
    progress: jobs.filter((job) => job.status.kind === "finished").length,
    total: jobs.length,
    teacher_name: courseSeeds[bookIndex % courseSeeds.length].teacher_name,
    chapters,
  };
});

// --- answers ----------------------------------------------------------

const answerStems = [
  "The teacher asked us to ________ the passage before class.",
  "Which of the following is the best title for the passage?",
  "听下面一段对话，选择正确答案。",
  "The more you practice, the ________ confident you will become.",
  "朗读下面的句子，注意重音和语调。",
  "What is the main idea of the second paragraph?",
  "Complete the sentence with the correct form of the word in brackets.",
  "根据短文内容，补全下面的表格。",
];

const answerStatuses: AnswerQuestion["status"][] = [
  "right",
  "right",
  "wrong",
  "partial",
  "unanswered",
  "right",
  "wrong",
  "right",
];

function buildAnswer(jobId: string, title: string): AnswerDocument {
  const seed = hash(jobId);
  const questions: AnswerQuestion[] = answerStems.map((stem, index) => {
    const status = answerStatuses[(seed + index) % answerStatuses.length];
    return {
      question_id: `${jobId}-q${index + 1}`,
      number: index + 1,
      type: index === 4 ? "9" : "1",
      stem,
      student_answer:
        status === "unanswered" ? "" : ["A", "B", "C", "D"][(seed + index) % 4],
      correct_answer: ["A", "B", "C", "D"][(seed + index + 1) % 4],
      score: status === "right" ? 5 : status === "partial" ? 2.5 : 0,
      max_score: 5,
      status,
    };
  });
  return {
    job_id: jobId,
    task_id: `content-${jobId}`,
    title,
    score: questions.reduce((sum, question) => sum + (question.score ?? 0), 0),
    max_score: questions.reduce((sum, question) => sum + question.max_score, 0),
    paper_xml: `<paper id="${jobId}"><section type="reading"><question number="1" type="1"/></section></paper>`,
    questions,
  };
}

// --- correction -------------------------------------------------------

let mockCorrectionRemaining = 12;
let mockCorrectionDraws = 0;
let mockDrawn: CorrectionQuestion[] = [];

const correctionSeeds: CorrectionQuestion[] = Array.from(
  { length: 6 },
  (_, index) => ({
    question_id: `mock-cq-${index + 1}`,
    number: String(index + 1),
    question_type: index % 3 === 0 ? "4" : "1",
    type_name: index % 3 === 0 ? "填空题" : "单项选择",
    score: index % 3 === 0 ? 1 : 2,
    stem: answerStems[index % answerStems.length],
    answer: index % 3 === 0 ? "assumptions" : "A. 第一项；B. 第二项",
  }),
);

// --- word pk ----------------------------------------------------------

const wordSeeds: Array<[string, string, string]> = [
  ["curious", "/ˈkjʊəriəs/", "好奇的"],
  ["challenge", "/ˈtʃælɪndʒ/", "挑战"],
  ["confident", "/ˈkɒnfɪdənt/", "自信的"],
  ["schedule", "/ˈʃedjuːl/", "日程安排"],
  ["volunteer", "/ˌvɒlənˈtɪə/", "志愿者"],
  ["improve", "/ɪmˈpruːv/", "改善"],
  ["debate", "/dɪˈbeɪt/", "辩论"],
  ["graduate", "/ˈɡrædʒueɪt/", "毕业"],
  ["prefer", "/prɪˈfɜː/", "更喜欢"],
  ["recommend", "/ˌrekəˈmend/", "推荐"],
  ["attitude", "/ˈætɪtjuːd/", "态度"],
  ["confuse", "/kənˈfjuːz/", "使困惑"],
];

function wordPkPreview(
  jobId: string,
  score: number,
  total: number,
): WordPkPreview {
  const job = jobs.find((row) => row.job_id === jobId);
  const pool = wordSeeds.map(([word, phonetic, translation], index) => ({
    word_id: `${jobId}-w${index + 1}`,
    word,
    phonetic,
    translation,
    audio_url: null,
  }));
  if (total <= 0 || score > total || total > pool.length) {
    throw {
      code: "invalid_input",
      message:
        total > pool.length
          ? `词库只有 ${pool.length} 个可用单词，不足以抽取 ${total} 个`
          : "分数不能大于 total",
      retryable: false,
    };
  }
  const picked = [...pool].sort(() => Math.random() - 0.5).slice(0, total);
  return {
    job_id: jobId,
    title: job?.name ?? "单词 PK",
    bucket_id: `bucket-${hash(jobId) % 1000}`,
    pool_size: pool.length,
    score,
    total,
    confirm_token: "mock-wordpk-token",
    words: picked.map((word, index) => ({ ...word, correct: index < score })),
  };
}

// --- read aloud -------------------------------------------------------

const readAloudSentences = [
  "The more you practice, the more confident you become.",
  "Reading aloud helps you remember new words.",
  "Our teacher encouraged us to join the debate club.",
  "I prefer to finish my homework before dinner.",
  "A good attitude is the first step towards success.",
];

const uploads = new Map<string, string>();

function readAloudTask(jobId: string): ReadAloudTask {
  const job = jobs.find((row) => row.job_id === jobId);
  return {
    job_id: jobId,
    title: job?.name ?? "课文跟读",
    submitted: false,
    slots: readAloudSentences.map((sentence, index) => {
      const questionId = `${jobId}-ra${index + 1}`;
      return {
        question_id: questionId,
        number: index + 1,
        prompt: "朗读下面的句子",
        sentence,
        audio_url: null,
        // Simulates a slot whose recording was already uploaded upstream.
        record_url: uploads.get(questionId) ?? null,
      };
    }),
    // Mirrors the backend: the submit path stays closed.
    submission_enabled: false,
  };
}

// --- settings ---------------------------------------------------------

let settings: Settings = {
  concurrency: 4,
  request_timeout_ms: 15_000,
  refresh_interval_sec: 300,
  read_aloud_dry_run: true,
  keep_audio_files: true,
};

// --- dispatcher -------------------------------------------------------

export async function handleMock(
  command: keyof CommandMap,
  args: unknown,
): Promise<unknown> {
  switch (command) {
    case "accounts_list": {
      await latency();
      return accounts.map((account) => ({ ...account }));
    }

    case "account_login": {
      const { username, password } = args as CommandMap["account_login"]["args"];
      await sleep(420);
      if (!username.trim() || !password) {
        throw { code: "auth_failed", message: "账号和密码不能为空", retryable: false };
      }
      const existing = accounts.find((item) => item.username === username.trim());
      if (!existing) {
        const created: Account = {
          account_id: `acc-${accounts.length + 1}`,
          username: username.trim(),
          display_name: username.trim(),
          school_name: "天学网实验中学",
          active: true,
          session_valid: true,
          expires_at: iso(30),
        };
        accounts = [created, ...accounts.map((item) => ({ ...item, active: false }))];
        return created;
      }
      accounts = accounts.map((item) => ({
        ...item,
        active: item.account_id === existing.account_id,
        session_valid:
          item.account_id === existing.account_id ? true : item.session_valid,
      }));
      return accounts.find((item) => item.account_id === existing.account_id)!;
    }

    case "account_switch": {
      const { account_id } = args as CommandMap["account_switch"]["args"];
      await latency();
      const found = accounts.find((item) => item.account_id === account_id);
      if (!found) {
        throw { code: "invalid_input", message: "账号不存在", retryable: false };
      }
      accounts = accounts.map((item) => ({
        ...item,
        active: item.account_id === account_id,
      }));
      return accounts.find((item) => item.account_id === account_id)!;
    }

    case "account_logout": {
      const { account_id } = args as CommandMap["account_logout"]["args"];
      await latency();
      accounts = accounts.map((item) =>
        item.account_id === account_id
          ? { ...item, active: false, session_valid: false }
          : item,
      );
      return null;
    }

    case "jobs_list": {
      await latency();
      return jobs.map((job) => ({ ...job }));
    }

    case "courses_list": {
      await latency();
      return courseSeeds.map((course) => ({ ...course }));
    }

    case "books_list": {
      await latency();
      return books.map((book) => ({
        book_id: book.book_id,
        name: book.name,
        subject: book.subject,
        cover_url: book.cover_url,
        is_2025_reader: book.is_2025_reader,
      }));
    }

    case "book_detail": {
      const { book_id } = args as CommandMap["book_detail"]["args"];
      await latency();
      const book = books.find((item) => item.book_id === book_id);
      if (!book) {
        throw { code: "not_found", message: "书籍不存在", retryable: false };
      }
      return book;
    }

    case "answer_get": {
      const { job_id } = args as CommandMap["answer_get"]["args"];
      await latency();
      const job = jobs.find((row) => row.job_id === job_id);
      return buildAnswer(job_id, job?.name ?? "作业答案");
    }

    case "wordpk_preview": {
      const { job_id, score, total } =
        args as CommandMap["wordpk_preview"]["args"];
      await latency();
      return wordPkPreview(job_id, score, total);
    }

    case "wordpk_submit": {
      await sleep(620);
      const { confirm_token } = args as CommandMap["wordpk_submit"]["args"];
      if (confirm_token !== "mock-wordpk-token") {
        throw {
          code: "confirmation_expired",
          message: "确认已过期或已使用，请重新抽取",
          retryable: false,
        };
      }
      return {
        ok: true,
        message: "提交成功（预览环境，未发送到服务器）。",
      } satisfies WordPkResult;
    }

    case "readaloud_get": {
      const { job_id } = args as CommandMap["readaloud_get"]["args"];
      await latency();
      return readAloudTask(job_id);
    }

    case "readaloud_upload": {
      const { recording } = args as CommandMap["readaloud_upload"]["args"];
      const { question_id, file_name } = recording;
      if (!file_name.toLowerCase().endsWith(".mp3")) {
        throw { code: "invalid_input", message: "只接受 mp3 文件", retryable: false };
      }
      await sleep(520);
      // Mirrors the backend: validated locally, never uploaded.
      return {
        ok: true,
        question_id,
        file_name,
        size: 1_024,
        record_url: null,
      } satisfies ReadAloudUpload;
    }

    case "readaloud_submit": {
      const { job_id } = args as CommandMap["readaloud_submit"]["args"];
      const { dry_run } = args as CommandMap["readaloud_submit"]["args"];
      await sleep(700);
      const task = readAloudTask(job_id);
      return {
        job_id,
        title: task.title,
        payload: { jobId: job_id, slots: task.slots },
        dry_run,
      } satisfies ReadAloudPreview;
    }

    case "correction_overview": {
      await latency();
      return {
        remaining_count: mockCorrectionRemaining,
        corrected_count: 482,
        today_corrected_count: 3,
        groups: [
          {
            group_key: "mock-group-1",
            remaining_count: mockCorrectionRemaining,
            qtype_name: "填空题",
            knowledge_name: "--",
          },
        ],
      } satisfies CorrectionOverview;
    }

    case "correction_draw": {
      await latency();
      const { count } = args as CommandMap["correction_draw"]["args"];
      if (mockCorrectionRemaining <= 0) {
        return {
          batch_id: "",
          path_prefix: "",
          questions: [],
          total_score: 0,
          confirm_token: null,
        } satisfies CorrectionDraw;
      }
      const questionCount = Math.min(Math.max(1, count), mockCorrectionRemaining);
      mockDrawn = correctionSeeds.slice(0, questionCount).map((question, index) => ({
        ...question,
        question_id: `${question.question_id}-${mockCorrectionDraws}`,
        number: String(index + 1),
      }));
      mockCorrectionDraws += 1;
      return {
        batch_id: `mock-batch-${mockCorrectionDraws}`,
        path_prefix: "",
        questions: mockDrawn,
        total_score: mockDrawn.reduce((sum, question) => sum + question.score, 0),
        confirm_token: "mock-correction-token",
      } satisfies CorrectionDraw;
    }

    case "correction_submit": {
      const { confirm_token } = args as CommandMap["correction_submit"]["args"];
      if (confirm_token !== "mock-correction-token" || mockDrawn.length === 0) {
        throw {
          code: "confirmation_expired",
          message: "确认已过期或已使用，请重新抽题",
          retryable: false,
        };
      }
      await sleep(360);
      mockCorrectionRemaining = Math.max(
        0,
        mockCorrectionRemaining - mockDrawn.length,
      );
      const submitted = mockDrawn.length;
      mockDrawn = [];
      return {
        ok: true,
        message: `已提交 ${submitted} 道题，订单号 mock-batch`,
        batch_id: "mock-batch",
        note_message: null,
      } satisfies CorrectionSubmit;
    }

    case "correction_status": {
      await latency();
      return {
        status: 2,
        percent: 100,
        submit_date: BASE,
      } satisfies CorrectionStatus;
    }

    case "settings_get": {
      await latency();
      return { ...settings };
    }

    case "settings_update": {
      const { settings: next } = args as CommandMap["settings_update"]["args"];
      await sleep(280);
      settings = { ...next };
      return { ...settings };
    }

    case "cache_clear": {
      await sleep(260);
      return null;
    }

    default: {
      const neverCommand: never = command;
      throw {
        code: "unknown_command",
        message: `未知命令：${String(neverCommand)}`,
        retryable: false,
      };
    }
  }
}
