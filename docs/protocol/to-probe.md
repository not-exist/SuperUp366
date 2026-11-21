# To probe

Items where the archive contradicts itself or lacks end-to-end confirmation.
Nothing here may be used for a real write until it is resolved and pinned by a
fixture.

## 1. Password salt

- `recon/api.md`: `fa&s*l%$k!fq$k!ld@fjlk`
- Several `tools/*.py`: `fa&s*l%$k!fq$k!ld:fjlk`

Both are exposed as `PASSWORD_SALT_AT` / `PASSWORD_SALT_COLON`. Resolve with one
real login; then delete the loser.

## 2. Task-score `ut`

Two candidate formulas:

- nested: `md5(md5(tasksJson) + "submitTaskToken-pc-987654" + ts)`
- flat:   `md5(tasksJson + "submitTaskToken-pc-987654" + ts)`

Relevant only if this client submits ordinary homework; read-aloud submission
uses the same signature path.

## 3. Word PK submit method

`recon/feature-word-pk.md` records **GET**; the retired Python client sent POST
first and fell back to GET on HTTP 404/405. Endpoint table currently says POST.

Resolve by observing which method the live server accepts. Do not "try both" in
production: a failed POST that still recorded the attempt would burn the task.

## 4. Read-aloud submission shape

Closed on 2026-11-09: two real homework submissions (one expired) were accepted
by branch A. The confirmed facts now live in `confirmed.md` § "Read-aloud".

## 5. Job ordering

Whether `allTaskList` array order equals the official UI order. If not, identify
the real sort key (`beginTime` / `endTime` / `displayOrder` / `taskNo`).

## 6. Composition detection

Applied writing (应用文) is resolved: the paper marks it with
`question_type = 14`, and `JobKind::AppliedWriting` is a title hint
(应用文 / 应用写作) reconciled against the paper. Mixed papers and 读后续写
(`question_type = 18`) remain out of scope for the writing page. Detection of
general 作文/续写 jobs is still unresolved.

## 7. Multi-account

Whether logging into several accounts on one device triggers a device limit
(`-31`), and whether sessions evict each other.

## 8. Course pagination

Whether `pager.pageSize=999` covers all accounts, or larger lists exist.

## 9. Correction submit payload

Pinned 2026-11-09: the gzip body, `submitType=11`, the `ut` formula,
`xmlResult.{answerdata,markdatao}` and the per-blank/per-parent mark split are
all verified on a real submission (see `confirmed.md`). Remaining unknowns:

1. `wrongnoteFlag` is written from the client-info `env` value; our client
   sends `"PRODUCT"` and the server accepted it, so it is at least not a gate.
2. Whether the server rejects `qstJson` rows for question types it does not
   expect. Per-blank types need `elementType=2` rows; choices work with only
   the parent row.
3. The `job/submit` ("错题消灭") follow-up is only exercised when the
   correction is opened with a `studyTaskId`; the standalone overview flow
   leaves it out.

## 10. Server time

`/front/current-time` availability, response shape and acceptable skew, needed
for the clock fields in submission payloads.

## 11. Recording types 12 and 13

`question_type = 9` (spoken follow-read) is confirmed end to end. Types `12`
(spoken Q&A) and `13` (retelling) are **recognised** because the official client
collects `urlList` entries for 9/12/13 and the book question rows report
`questionTypeNum` 9/12/13 as recordings (**[R]**), and this client submits them
through the same `/client/task/score/submit/v2` row with their own
`question_type` in `qstJson`. No live submission of a 12/13 paper has been
observed yet, so a server-side rejection (or a different XML shape for those
types) stays an open risk until one is recorded.

## 12. Course-book page submission (live)

Static evidence from the official PC reader says a class-book page submits via
`/client/task/score/submit/v2` with empty `studyTaskId`, `submitType = 0`, empty
`qstJson`, `taskId = page id`, `chapterId = page.parentId`, and XML in `result`.
The upload worker parses `extendParams` from JSON text before sending; an empty
value becomes the object `{}`. These fields are required to associate the row with
the page's chapter/task context.

Live attempt on 2026-11-21 for 新概念英语1, Lesson 1 `课文听读`
(`bookId=B8FB77D2AAAEE151EE2DA265EB9CA6D1`, `courseId=9222783`,
`taskId=DE3C6927A54C4FF17EFB5ACCBA00E09C`, `paperId=C7C46EB92FA4832F84D38588656F7449`):

- The account's book metadata reports `packType=0`, `bookType=2`; the page is
  `pageType=0`, `type=1`, `isContent=1` and contains eight type-9 questions.
- One textbook-bridge v2 request with `srcType=2`, `submitType=0`, empty
  `studyTaskId` and `qstJson`, the page id in both `taskId` and `chapterId`, and
  the constructed XML `result` returned success
  (`data` absent). Do not retry this
  task with the same or a guessed alternate shape.
- Read-only checks afterward still showed no `finished-task/list` row, the task
  node at score 0, all eight question rows at score 0 with empty `myAnswer`, and
  `answer/detail.userAnswer` as `<elements></elements>`. This establishes that
  the v2 attempt did not persist a visible answer; it does not establish whether
  the endpoint silently ignored the row or accepted it in another subsystem.
- The eight supplied MP3 files uploaded to OSS and are retained in the local
  `audio_assets` table with course-scoped keys and measured duration. OSS has no
  delete endpoint in the confirmed API inventory.

- Static extraction distinguishes two PC branches: `submitAnswerData("textbook")`
  uses `srcType=2` plus XML `result`, while the `packType < 7` flipbook path
  uses `submitTaskV1`/`srcType=1`, an empty `result`, and qstJson previously
  populated by `submitEleAttr`. A dry-run for the target produced 21 rows
  (8 question rows plus 13 sentence rows). The first client implementation sent
  the page id as both `taskId` and `chapterId` and sent legacy `extendParams` as
  an empty string; both differ from the official page context/worker normalization.
  The corrected chapter association is covered by offline tests. The renderer's
  exact ASES score attributes and runtime `StudyTaskId/taskNo` remain unresolved,
  so no further live write is allowed without new explicit approval.
