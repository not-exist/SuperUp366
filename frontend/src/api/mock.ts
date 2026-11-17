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
  AppliedWritingPreview,
  AppliedWritingSubmit,
  AppliedWritingTask,
  Book,
  BookDetail,
  BookJob,
  BookTaskDetail,
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
  ReadAloudSubmit,
  ReadAloudTask,
  ReadAloudUpload,
  Settings,
  WordPkPreview,
  WordPkResult,
} from "./types";

const DAY = 86_400_000;
const BASE = Date.parse("2026-11-07T09:00:00.000Z");

/**
 * `vite build --mode e2e` swaps this to true. The Playwright suite relies on
 * it: delays collapse to zero and `window.__mock` exposes reset and failure
 * injection. Production builds never define it.
 */
const E2E_MODE = import.meta.env.MODE === "e2e";

function iso(dayOffset: number, hour = 20): string {
  return new Date(BASE + dayOffset * DAY + (hour - 9) * 3_600_000).toISOString();
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Simulated latency; skipped in e2e so assertions do not wait on timers. */
function delay(ms: number): Promise<void> {
  return E2E_MODE ? Promise.resolve() : sleep(ms);
}

function latency(): Promise<void> {
  return E2E_MODE ? Promise.resolve() : sleep(120 + Math.floor(Math.random() * 160));
}

function hash(value: string): number {
  let h = 2166136261;
  for (let i = 0; i < value.length; i += 1) {
    h ^= value.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h);
}

/** Deterministic Fisher-Yates; `Math.random` would make e2e flaky. */
function seededShuffle<T>(items: T[], seed: number): T[] {
  const copy = [...items];
  let state = seed || 1;
  for (let i = copy.length - 1; i > 0; i -= 1) {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    const j = state % (i + 1);
    [copy[i], copy[j]] = [copy[j], copy[i]];
  }
  return copy;
}

// --- accounts ---------------------------------------------------------

function seedAccounts(): Account[] {
  return [
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
}

let accounts: Account[] = seedAccounts();

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
  "应用文写作：My Weekend",
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
      const kind = name.includes("单词卡")
        ? "word_pk"
        : name.includes("应用文")
          ? "applied_writing"
          : "ordinary";
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

let jobs: JobRow[] = buildJobs();

// --- books ------------------------------------------------------------

const bookSeeds: Book[] = [
  {
    book_id: "book-1",
    name: "英语必修 第一册",
    subject: "英语",
    cover_url: null,
    is_2025_reader: false,
    course_id: null,
    book_type: 1,
  },
  {
    book_id: "book-2",
    name: "英语必修 第二册",
    subject: "英语",
    cover_url: null,
    is_2025_reader: false,
    course_id: null,
    book_type: 1,
  },
  {
    book_id: "book-3",
    name: "高中英语阅读进阶",
    subject: "英语",
    cover_url: null,
    is_2025_reader: true,
    course_id: null,
    book_type: 1,
  },
  {
    book_id: "book-4",
    name: "新概念英语1",
    subject: "英语",
    cover_url: null,
    is_2025_reader: false,
    course_id: "course-1",
    book_type: 2,
  },
];

const bookChapterTitles = [
  "Unit 1 Teenage Life",
  "Unit 2 Travelling Around",
  "Unit 3 Sports and Fitness",
];

function bookJob(
  bookId: string,
  bookType: number,
  chapterIndex: number,
  jobIndex: number,
): BookJob {
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
    // An ordinary book joins the recording task to the seeded read-aloud job.
    // A class book's page has no job row; it uses the book read-aloud route.
    readaloud_job_id: jobIndex === 1 && bookType !== 2 ? "job-1-9" : null,
    readaloud_book: jobIndex === 1 && bookType === 2,
  };
}

const books: BookDetail[] = bookSeeds.map((book, bookIndex) => {
  const chapters = bookChapterTitles.map((title, chapterIndex) => ({
    chapter_id: `${book.book_id}-ch-${chapterIndex + 1}`,
    title,
    jobs: Array.from({ length: 4 }, (_, jobIndex) =>
      bookJob(book.book_id, book.book_type, chapterIndex, jobIndex),
    ),
  }));
  const jobs = chapters.flatMap((chapter) => chapter.jobs);
  return {
    book_id: book.book_id,
    name: book.name,
    subject: book.subject,
    cover_url: book.cover_url,
    is_2025_reader: book.is_2025_reader,
    // A class-assigned book keeps its seed course; purchased books fall back
    // to a course because the detail endpoint needs one.
    course_id:
      book.course_id ?? courseSeeds[bookIndex % courseSeeds.length].course_id,
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
    applied_writing: false,
  };
}

// --- correction -------------------------------------------------------

let correctionRemaining = 12;
let correctionDraws = 0;
let drawnQuestions: CorrectionQuestion[] = [];
/** Full shelf override; `null` means "use the seeds". */
let booksOverride: Book[] | null = null;

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
  const picked = seededShuffle(pool, hash(`${jobId}:${score}:${total}`)).slice(
    0,
    total,
  );
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
  const slots = readAloudSentences.map((sentence, index) => {
    const questionId = `${jobId}-ra${index + 1}`;
    return {
      question_id: questionId,
      number: index + 1,
      sentence,
      sentences: [sentence],
      score: 10,
      duration: 90,
      audio_url: null,
      // Simulates a slot whose recording was already uploaded.
      record_url: uploads.get(questionId) ?? null,
    };
  });
  return {
    job_id: jobId,
    title: job?.name ?? "课文跟读",
    submitted: false,
    expired: false,
    question_count: slots.length,
    sentence_count: slots.length,
    total_score: slots.reduce((sum, slot) => sum + slot.score, 0),
    slots,
  };
}

// --- settings ---------------------------------------------------------

const DEFAULT_SETTINGS: Settings = {
  concurrency: 4,
  request_timeout_ms: 15_000,
  refresh_interval_sec: 300,
  keep_audio_files: true,
};

let settings: Settings = { ...DEFAULT_SETTINGS };

// --- read-aloud helpers ----------------------------------------------

async function mockUpload(recording: {
  question_id: string;
  file_name: string;
}): Promise<ReadAloudUpload> {
  const { question_id, file_name } = recording;
  if (!file_name.toLowerCase().endsWith(".mp3")) {
    throw { code: "invalid_input", message: "只接受 mp3 文件", retryable: false };
  }
  await delay(520);
  const record_url = `https://mock.oss/${question_id}.mp3`;
  uploads.set(question_id, record_url);
  return {
    ok: true,
    question_id,
    file_name,
    size: 1_024,
    record_url,
  } satisfies ReadAloudUpload;
}

function mockPreview(
  taskId: string,
  scores?: { question_id: string; score: number }[],
): ReadAloudPreview {
  const task = readAloudTask(taskId);
  const slots = task.slots.map((slot) => {
    const score =
      scores?.find((entry) => entry.question_id === slot.question_id)?.score ??
      slot.score;
    return {
      question_id: slot.question_id,
      number: slot.number,
      sentence: slot.sentence,
      score,
      record_url: slot.record_url ?? "",
    };
  });
  if (slots.some((slot) => !slot.record_url)) {
    throw { code: "invalid_input", message: "还有题目没有上传录音", retryable: false };
  }
  return {
    job_id: taskId,
    title: task.title,
    total_score: task.total_score,
    reported_score: slots.reduce((sum, slot) => sum + slot.score, 0),
    // A book page has no job id, so its row carries an empty `studyTaskId`
    // and `submitType = 0`, matching the reader's textbook branch.
    row: taskId.startsWith("book-")
      ? { studyTaskId: "", submitType: 0, qstJson: "" }
      : { studyTaskId: taskId, qstJson: [] },
    slots,
    confirm_token: "mock-readaloud-token",
  } satisfies ReadAloudPreview;
}

// --- applied writing --------------------------------------------------

const submittedWritingJobs = new Set<string>();
const pendingWritingJobs = new Map<string, string>();
let writingPreviewCount = 0;

function appliedWritingTask(jobId: string): AppliedWritingTask {
  const job = jobs.find((row) => row.job_id === jobId);
  return {
    job_id: jobId,
    title: job?.name ?? "应用文写作",
    submitted: submittedWritingJobs.has(jobId),
    expired: false,
    total_score: 15,
    questions: [
      {
        question_id: `${jobId}-w1`,
        number: "1",
        score: 15,
        prompt:
          "假定你是李华，你的英国朋友 Chris 想了解你校的体育活动。\n请你写一封邮件，内容包括：\n1. 你参加的体育活动；\n2. 你的感受。\n注意：词数 80 左右。",
        word_num: 80,
        duration: 1200,
      },
    ],
  };
}

function mockWritingPreview(
  jobId: string,
  answers: { question_id: string; text: string }[],
): AppliedWritingPreview {
  const task = appliedWritingTask(jobId);
  const token = `mock-writing-token-${++writingPreviewCount}`;
  pendingWritingJobs.set(token, jobId);
  if (answers.length !== task.questions.length) {
    throw { code: "invalid_input", message: "请先填写全部作文", retryable: false };
  }
  const elements = task.questions.map((question) => {
    const answer = answers.find((entry) => entry.question_id === question.question_id);
    if (!answer || !answer.text.trim()) {
      throw { code: "invalid_input", message: "作文不能为空", retryable: false };
    }
    return `<element id="${question.question_id}"><answers><![CDATA[${answer.text.trim()}]]></answers></element>`;
  });
  return {
    job_id: jobId,
    title: task.title,
    total_score: task.total_score,
    answers: task.questions.map((question) => {
      const answer = answers.find((entry) => entry.question_id === question.question_id);
      return {
        question_id: question.question_id,
        number: question.number,
        text: answer?.text.trim() ?? "",
      };
    }),
    answer_data: `<elements>${elements.join("")}</elements>`,
    confirm_token: token,
  };
}

// --- dispatcher -------------------------------------------------------

export async function handleMock(
  command: keyof CommandMap,
  args: unknown,
): Promise<unknown> {
  const injected = takeFailure(command);
  if (injected) {
    throw injected;
  }

  switch (command) {
    case "accounts_list": {
      await latency();
      return accounts.map((account) => ({ ...account }));
    }

    case "account_login": {
      const { username, password } = args as CommandMap["account_login"]["args"];
      await delay(420);
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
      if (booksOverride) {
        return booksOverride.map((book) => ({ ...book }));
      }
      return books.map((book) => ({
        book_id: book.book_id,
        name: book.name,
        subject: book.subject,
        cover_url: book.cover_url,
        is_2025_reader: book.is_2025_reader,
        book_type:
          bookSeeds.find((seed) => seed.book_id === book.book_id)?.book_type ??
          1,
        course_id: book.course_id,
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

    case "book_task_get": {
      const { task_id, word_task, book_id } =
        args as CommandMap["book_task_get"]["args"];
      await latency();
      const job = books
        .find((book) => book.book_id === book_id)
        ?.chapters.flatMap((chapter) => chapter.jobs)
        .find((item) => item.job_id === task_id);
      if (!job) {
        throw { code: "not_found", message: "任务不存在", retryable: false };
      }
      if (word_task) {
        const result: BookTaskDetail = {
          kind: "word_pk",
          read_aloud: false,
          questions: [],
          words: ["apple", "bridge", "culture", "journey"].map((word, index) => ({
            word,
            mastered: index % 2 === 0,
          })),
        };
        return result;
      }
      const result: BookTaskDetail = {
        kind: "ordinary",
        read_aloud: job.read_aloud,
        words: [],
        questions: Array.from({ length: 4 }, (_, index) => ({
          question_id: `${task_id}-q${index + 1}`,
          number: String(index + 1),
          type_name: job.read_aloud ? "口语跟读" : "单选题",
          knowledge: job.name,
          my_answer: job.read_aloud ? "https://example.com/recording.mp3" : "A",
          correct_answer: job.read_aloud ? "" : "A",
          max_score: 10,
          score: job.read_aloud ? 8.5 : 10,
          class_average: 7.9,
          read_aloud: job.read_aloud,
        })),
      };
      return result;
    }

    case "answer_get": {
      const { job_id } = args as CommandMap["answer_get"]["args"];
      await latency();
      const job = jobs.find((row) => row.job_id === job_id);
      const answer = buildAnswer(job_id, job?.name ?? "作业答案");
      answer.applied_writing = job?.kind === "applied_writing";
      return answer;
    }

    case "wordpk_preview": {
      const { job_id, score, total } =
        args as CommandMap["wordpk_preview"]["args"];
      await latency();
      return wordPkPreview(job_id, score, total);
    }

    case "wordpk_submit": {
      await delay(620);
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
      return mockUpload(recording);
    }

    case "readaloud_preview": {
      const { job_id, scores } = args as CommandMap["readaloud_preview"]["args"];
      await delay(420);
      return mockPreview(job_id, scores);
    }

    case "book_readaloud_get": {
      const { task_id } = args as CommandMap["book_readaloud_get"]["args"];
      await latency();
      return readAloudTask(task_id);
    }

    case "book_readaloud_upload": {
      const { recording } = args as CommandMap["book_readaloud_upload"]["args"];
      return mockUpload(recording);
    }

    case "book_readaloud_preview": {
      const { task_id, scores } =
        args as CommandMap["book_readaloud_preview"]["args"];
      await delay(420);
      return mockPreview(task_id, scores);
    }

    case "readaloud_submit": {
      const { confirm_token } = args as CommandMap["readaloud_submit"]["args"];
      await delay(700);
      if (confirm_token !== "mock-readaloud-token") {
        throw {
          code: "confirmation_expired",
          message: "确认已过期或已使用，请重新预览",
          retryable: false,
        };
      }
      return {
        ok: true,
        verified: true,
        score: 30,
        batch_id: "mock-batch",
        message: "提交成功（预览环境，未发送到服务器）。",
      } satisfies ReadAloudSubmit;
    }

    case "applied_writing_get": {
      const { job_id } = args as CommandMap["applied_writing_get"]["args"];
      await latency();
      return appliedWritingTask(job_id);
    }

    case "applied_writing_preview": {
      const { job_id, answers } =
        args as CommandMap["applied_writing_preview"]["args"];
      await delay(420);
      return mockWritingPreview(job_id, answers);
    }

    case "applied_writing_submit": {
      const { confirm_token } = args as CommandMap["applied_writing_submit"]["args"];
      await delay(700);
      const jobId = pendingWritingJobs.get(confirm_token);
      if (jobId === undefined) {
        throw {
          code: "confirmation_expired",
          message: "确认已过期或已使用，请重新预览",
          retryable: false,
        };
      }
      pendingWritingJobs.delete(confirm_token);
      const submitted = jobs.find((row) => row.job_id === jobId);
      if (submitted) {
        submittedWritingJobs.add(submitted.job_id);
        submitted.status = { kind: "finished", score: 12 };
        submitted.score = 12;
      }
      return {
        ok: true,
        verified: true,
        score: 12,
        message: "提交成功（预览环境，未发送到服务器）。",
      } satisfies AppliedWritingSubmit;
    }

    case "correction_overview": {
      await latency();
      return {
        remaining_count: correctionRemaining,
        corrected_count: 482,
        today_corrected_count: 3,
        groups: [
          {
            group_key: "mock-group-1",
            remaining_count: correctionRemaining,
            qtype_name: "填空题",
            knowledge_name: "--",
          },
        ],
      } satisfies CorrectionOverview;
    }

    case "correction_draw": {
      await latency();
      const { count } = args as CommandMap["correction_draw"]["args"];
      if (correctionRemaining <= 0) {
        return {
          batch_id: "",
          path_prefix: "",
          questions: [],
          total_score: 0,
          confirm_token: null,
        } satisfies CorrectionDraw;
      }
      const questionCount = Math.min(Math.max(1, count), correctionRemaining);
      drawnQuestions = correctionSeeds.slice(0, questionCount).map((question, index) => ({
        ...question,
        question_id: `${question.question_id}-${correctionDraws}`,
        number: String(index + 1),
      }));
      correctionDraws += 1;
      return {
        batch_id: `mock-batch-${correctionDraws}`,
        path_prefix: "",
        questions: drawnQuestions,
        total_score: drawnQuestions.reduce((sum, question) => sum + question.score, 0),
        confirm_token: "mock-correction-token",
      } satisfies CorrectionDraw;
    }

    case "correction_submit": {
      const { confirm_token } = args as CommandMap["correction_submit"]["args"];
      if (confirm_token !== "mock-correction-token" || drawnQuestions.length === 0) {
        throw {
          code: "confirmation_expired",
          message: "确认已过期或已使用，请重新抽题",
          retryable: false,
        };
      }
      await delay(360);
      correctionRemaining = Math.max(
        0,
        correctionRemaining - drawnQuestions.length,
      );
      const submitted = drawnQuestions.length;
      drawnQuestions = [];
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
      await delay(280);
      settings = { ...next };
      return { ...settings };
    }

    case "cache_clear": {
      await delay(260);
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

// --- e2e control surface ---------------------------------------------
//
// Compiled only into `vite build --mode e2e`. Tests use it to force error
// states and empty shelves without reaching the real backend.

export interface MockError {
  code: string;
  message: string;
  retryable: boolean;
}

export interface MockBootstrap {
  /** Replace the stored account list (an empty array shows first-run login). */
  accounts?: Account[];
  /** Replace the job list; an empty array exercises the empty state. */
  jobs?: JobRow[];
  /** Replace the shelf; an empty array exercises the empty state. */
  books?: Book[];
  settings?: Settings;
  correctionRemaining?: number;
  /** Commands that fail once, from the very first render. */
  failOnStart?: Partial<Record<keyof CommandMap, MockError>>;
  /**
   * Commands that keep failing until `clearFailures()`. Queries retry once by
   * default, so a single failure would be invisible; use this for error-state
   * tests and clear it before asserting the retry path.
   */
  failAlways?: Partial<Record<keyof CommandMap, MockError>>;
}

export interface MockControl {
  reset(options?: MockBootstrap): void;
  failNext(command: keyof CommandMap, error?: Partial<MockError>): void;
  clearFailures(): void;
}

declare global {
  interface Window {
    __mock?: MockControl;
    __MOCK_BOOTSTRAP__?: MockBootstrap;
  }
}

const injectedFailures = new Map<keyof CommandMap, MockError[]>();
const persistentFailures = new Map<keyof CommandMap, MockError>();

function takeFailure(command: keyof CommandMap): MockError | undefined {
  return (
    injectedFailures.get(command)?.shift() ?? persistentFailures.get(command)
  );
}

function clearFailures(): void {
  injectedFailures.clear();
  persistentFailures.clear();
}

function seedDefaults(settings: Settings): Settings {
  return { ...settings };
}

/** Restore every mutable value to its seed, then apply overrides. */
export function resetMock(options?: MockBootstrap): void {
  accounts = seedAccounts();
  jobs = buildJobs();
  booksOverride = null;
  uploads.clear();
  settings = seedDefaults(DEFAULT_SETTINGS);
  correctionRemaining = 12;
  correctionDraws = 0;
  drawnQuestions = [];
  submittedWritingJobs.clear();
  pendingWritingJobs.clear();
  writingPreviewCount = 0;
  clearFailures();
  applyBootstrap(options);
}

function applyBootstrap(options?: MockBootstrap): void {
  if (!options) return;
  if (options.accounts) {
    accounts = options.accounts.map((account) => ({ ...account }));
  }
  if (options.jobs) {
    jobs = options.jobs.map((job) => ({ ...job }));
  }
  if (options.books) {
    booksOverride = options.books.map((book) => ({ ...book }));
  }
  if (options.settings) {
    settings = seedDefaults(options.settings);
  }
  if (typeof options.correctionRemaining === "number") {
    correctionRemaining = options.correctionRemaining;
  }
  for (const [command, error] of Object.entries(options.failOnStart ?? {})) {
    injectFailure(command as keyof CommandMap, error as MockError);
  }
  for (const [command, error] of Object.entries(options.failAlways ?? {})) {
    persistentFailures.set(
      command as keyof CommandMap,
      normalizeError(error as MockError),
    );
  }
}

function normalizeError(error: Partial<MockError>): MockError {
  return {
    code: error.code ?? "network_error",
    message: error.message ?? "模拟网络错误（e2e）",
    retryable: error.retryable ?? true,
  };
}

function injectFailure(
  command: keyof CommandMap,
  error: Partial<MockError> = {},
): void {
  const queue = injectedFailures.get(command) ?? [];
  queue.push(normalizeError(error));
  injectedFailures.set(command, queue);
}

if (E2E_MODE) {
  applyBootstrap(window.__MOCK_BOOTSTRAP__);
  window.__mock = {
    reset: resetMock,
    failNext: injectFailure,
    clearFailures,
  };
}
