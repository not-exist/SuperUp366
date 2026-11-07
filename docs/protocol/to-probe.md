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

Which branch applies to a given task:

- `POST /client/task/score/submit/v2` with `submitType=8`
- `POST /client/onlinetesting/submit`
- `POST /client/task/score/gzip/submit`

Also unresolved: exact `mark_datao` element numbering, read-aloud `qstJson`
child shape, and whether the server asynchronously re-grades (observed score
changes after submission). Until resolved, the UI must stop at upload
verification + dry-run.

## 5. Job ordering

Whether `allTaskList` array order equals the official UI order. If not, identify
the real sort key (`beginTime` / `endTime` / `displayOrder` / `taskNo`).

## 6. Wrong-note cleanup granularity

Whether `question/submit` accepts one question per call, or only a whole page.
If only whole-page, the batch model degrades to one submission per page and the
preview must say so.

## 7. Essay detection

How to reliably identify composition (作文) jobs so they are excluded from the
answer view. Current code leaves them `JobKind::Unknown` until confirmed.

## 8. Multi-account

Whether logging into several accounts on one device triggers a device limit
(`-31`), and whether sessions evict each other.

## 9. Course pagination

Whether `pager.pageSize=999` covers all accounts, or larger lists exist.

## 10. Server time

`/front/current-time` availability, response shape and acceptable skew, needed
for `startTime`/`endTime` in wrong-note submission.
