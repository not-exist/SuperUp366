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

Confirmed by the archived live-payload analysis on `main`
(`docs/plans/2026-11-03-readaloud-mp3-score-submit.md`, F1–F16):

- Recording unit is the **question**, not the sentence.
- Audio is uploaded to OSS first; the link the server reads lives in the paper
  XML `<answer_url>` and in `qstJson[].elementAttr.record_url`. There is no
  `objectIds`/`uuid` on the task row itself.
- `objectName` is generated locally as
  `storage{userType}/study/a/{yyyy/mm/dd}/{uid}/{file}`.
- `ut` = `md5(ts)[..10] + md5(md5(payload) + "submitTaskToken-pc-987654" +
  md5(ts)) + md5(ts)[10..]`, 64 chars.
- OSS credentials come from `GET /front/aliyunoss/token/v2?organId={schoolId}`
  and the upload is a `multipartUpload`.

Still to pin for this client:

1. Which of the three branches a given read-aloud task uses (A
   `/client/task/score/submit/v2` with `submitType=8`, B
   `/client/onlinetesting/submit` with no `ut`, C the gzip variant), since the
   archived payloads are textbook tasks and may not cover homework.
2. The exact `mark_datao` element numbering and read-aloud `qstJson` child
   shape for a **homework** task.
3. Whether the server asynchronously re-grades after submission (the archive
   observed score changes), which must be reflected in the UI wording.

Until 1–3 are pinned against a real homework task, the app stops at upload
verification and dry-run: uploading audio is safe and reversible, submitting a
score is not.

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
