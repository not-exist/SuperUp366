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

## 6. Essay detection

How to reliably identify composition (作文) jobs so they are excluded from the
answer view. Current code leaves them `JobKind::Unknown` until confirmed.

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
