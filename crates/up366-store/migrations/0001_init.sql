-- Initial schema. All timestamps are Unix epoch milliseconds (INTEGER).
-- Account ids are opaque strings owned by the application, not by upstream.

CREATE TABLE accounts (
    id            TEXT PRIMARY KEY NOT NULL,
    username      TEXT UNIQUE,
    display_name  TEXT,
    uid           TEXT,
    school_id     TEXT,
    user_type     INTEGER,
    client_id     TEXT,
    status        TEXT NOT NULL DEFAULT 'active',
    last_login_at INTEGER,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);

-- Courses are a cache of the upstream course list, scoped per account.
CREATE TABLE courses (
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    course_id   TEXT NOT NULL,
    name        TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    raw_json    TEXT NOT NULL,
    fetched_at  INTEGER NOT NULL,
    PRIMARY KEY (account_id, course_id)
);
CREATE INDEX idx_courses_fetched ON courses (account_id, fetched_at);

-- Jobs are cached per (account, course); a job may appear under several courses.
CREATE TABLE jobs (
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    course_id   TEXT NOT NULL,
    job_id      TEXT NOT NULL,
    content_id  TEXT,
    book_id     TEXT,
    chapter_id  TEXT,
    name        TEXT NOT NULL,
    order_index INTEGER NOT NULL,
    raw_json    TEXT NOT NULL,
    fetched_at  INTEGER NOT NULL,
    PRIMARY KEY (account_id, course_id, job_id)
);
CREATE INDEX idx_jobs_account_fetched ON jobs (account_id, fetched_at);
CREATE INDEX idx_jobs_account_course ON jobs (account_id, course_id, order_index);

-- Latest known upstream status for a job.
CREATE TABLE job_status (
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    job_id      TEXT NOT NULL,
    task_status INTEGER,
    stu_status  INTEGER,
    submit_time INTEGER,
    score       REAL,
    max_score   REAL,
    fetched_at  INTEGER NOT NULL,
    PRIMARY KEY (account_id, job_id)
);

-- Purchased books.
CREATE TABLE books (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    book_id    TEXT NOT NULL,
    name       TEXT,
    subject    TEXT,
    raw_json   TEXT NOT NULL,
    fetched_at INTEGER NOT NULL,
    PRIMARY KEY (account_id, book_id)
);

-- Per-question answers, either fetched from upstream or composed locally.
CREATE TABLE answers (
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    job_id      TEXT NOT NULL,
    question_id TEXT NOT NULL,
    content_id  TEXT,
    answer_json TEXT NOT NULL,
    score       REAL,
    max_score   REAL,
    fetched_at  INTEGER NOT NULL,
    PRIMARY KEY (account_id, job_id, question_id)
);

-- Local audio recordings and their OSS destinations.
CREATE TABLE audio_assets (
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    asset_id    TEXT NOT NULL,
    job_id      TEXT,
    question_id TEXT,
    local_path  TEXT NOT NULL,
    object_name TEXT,
    size        INTEGER NOT NULL,
    sha256      TEXT NOT NULL,
    duration_ms INTEGER,
    created_at  INTEGER NOT NULL,
    PRIMARY KEY (account_id, asset_id)
);
CREATE INDEX idx_audio_assets_job ON audio_assets (account_id, job_id);

-- Outbox for submissions, so a crash mid-submit is recoverable.
CREATE TABLE submissions (
    id           TEXT PRIMARY KEY NOT NULL,
    account_id   TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    job_id       TEXT NOT NULL,
    kind         TEXT NOT NULL,
    status       TEXT NOT NULL,
    payload_json TEXT,
    error        TEXT,
    submitted_at INTEGER,
    created_at   INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL
);
CREATE INDEX idx_submissions_account_job ON submissions (account_id, job_id);

-- Wrong-note cache.
CREATE TABLE wrongnote_items (
    account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    question_id     TEXT NOT NULL,
    subject         TEXT,
    knowledge_point TEXT,
    content_json    TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    fetched_at      INTEGER NOT NULL,
    PRIMARY KEY (account_id, question_id)
);

-- One row per wrong-note clean run, with its per-question decisions.
CREATE TABLE wrongnote_clean_runs (
    id          TEXT PRIMARY KEY NOT NULL,
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    status      TEXT NOT NULL,
    total       INTEGER NOT NULL DEFAULT 0,
    removed     INTEGER NOT NULL DEFAULT 0,
    started_at  INTEGER NOT NULL,
    finished_at INTEGER,
    error       TEXT
);

CREATE TABLE wrongnote_clean_items (
    run_id      TEXT NOT NULL REFERENCES wrongnote_clean_runs(id) ON DELETE CASCADE,
    question_id TEXT NOT NULL,
    action      TEXT NOT NULL,
    result      TEXT,
    PRIMARY KEY (run_id, question_id)
);

-- Application settings. Global (not per account); values are opaque strings.
CREATE TABLE settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
