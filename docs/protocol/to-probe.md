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

Read from the official reader bundle and the PC main process: a class-book page
submits through `/client/task/score/submit/v2` (branch A) with an empty
`studyTaskId`, `submitType = 0`, no `qstJson`, `taskId = chapterId = page id`,
and the answer XML in `result`. The book tree -> `paperId` ->
`textbook/paperinfo` path is verified read-only (§ "Course-book pages" in
`confirmed.md`), but no live submission from this client has been recorded.

Resolve with one 新概念英语1 page chosen by the account owner: after the POST,
`/client/book/finished-task/list?bookId=` must gain a row keyed by the page id
and the book detail progress must move. If the server rejects the empty
`studyTaskId`, the failure code is the deciding evidence — do not retry with a
self-made id, which writes a score row nothing associates with the page.

## 13. Dictation / memorisation homework

Candidate metadata observed in cached jobs includes `xotParam.wordDictationType
= 0` (including `paperType = 2` examples) and `paperType = 5` examples without
that flag. These are discovery hints only; neither field is established as an
authoritative type or as a submission-mode selector. One sampled title-matched
paper is documented in `confirmed.md` as `question_type = 19` (篇章默写), but
that observation does not define all 默写 / 听写 / 单词拼写 tasks.

Additional read-only evidence gathered 2026-11-17:

- A live type-19 paper was retrieved through `paper/detail`; its paper XML
  contains one `<element type="3">` with `<question_type>19</question_type>`.
  `answer/detail/v2` for that same content ID returned `paperInfo.paperData`,
  `paperInfo.answerData`, `userAnswer.answerData`, and `userAnswer.markData`.
  The stored student answer is normalized XML containing an `<answers>` node
  and `<pictures>` node; `markData` is a separate document. These are response
  shapes, not proof of the original submit request shape.
- `job/info` for that task reports `taskStatus=1`, `stuStatus=2`, and a
  `submitTime`; the stale local job cache had reported `taskStatus=0`. This
  confirms status can lag in cache, not that `reTest=1` permits another write.
- `POST /client/onlinetesting/alldata` with the task `contentId` and uid returns
  `code=1` / `测验不存在!`. This ID is not accepted as that endpoint's `testId`;
  the result does not establish whether `/client/onlinetesting/submit` accepts
  the same job, nor does it identify an alternative submit endpoint.
- The official-client recon describes `/client/onlinetesting/submit` as the
  whole-document homework route and `/client/task/score/submit/v2` as the
  per-question textbook route. Its recorded `getPaperXML` implementation is
  for applied-writing type 14 only. The saved real-submit fixtures cover type 4
  on the gzip task-score route; none contain type 19.

Resolved by client material on 2026-11-18 (all **[R]**, no live write):

- The type-19/20 serialiser is `getQstXML`/`getPaperXML`:
  `<element id="…"><answers><![CDATA[answerText]]></answers></element>`, with no
  `<pictures>`; an empty answer drops the element. `markData` is built only from
  a question's `pigaiData`, so an ordinary homework submit carries none.
- The homework page submits the whole document to
  `xot-api /client/onlinetesting/submit` with
  `{testId, paperId, uid, answerData, duration}` (form-urlencoded, no `ut`, no
  gzip). Inside the app shell the native uploader adds
  `beginTime`/`endTime`/`playData`/`timeData`. `testId` is the job's
  `contentId`.
- The APK has no native dictation or redo path: 默写/听写/重做 do not occur in its
  resources, and the same H5 homework page runs in
  `com.up366.mobile.course.task.JsSdk2023TaskActivity`. The redo call is made by
  the page's JavaScript, which is why the endpoint is absent from the APK's
  (partly encrypted) URL pool.
- Retest has a dedicated endpoint:
  `homework-api /client/homework/test/redo` with `{testId, returnType: "1"}`,
  enabled only when `onlinemarking/alldata → testInfo.parameters.reTest == 1`.
  Codes: 0 accepted, 1 already closed, anything else unsupported.
- No other resubmit route exists. `reTest` also appears on never-submitted jobs
  (88 of 173), so it is a template/config flag, not a permission.

Still unverified, and the reason to keep the feature read-only:

1. No end-to-end live run on a job that actually has an XOT test record. The
   re-login on 2026-11-18 succeeded, but every submitted dictation candidate in
   this account is a photo/custom-sheet assignment: `onlinemarking/alldata` and
   `onlinetesting/alldata` answer `code 1 测验不存在` for `contentId`,
   `jobId` and `paperId`, `homework/test/info` answers `-3 no_data`, and
   `answer/detail/v2` has no stored answer. The reset plus second submit stays
   unobserved until a job with a live test record is available (or the job that
   was redone in the Android app is named).
2. `onlinetesting/alldata` `{testId: contentId, uid}` is still unconfirmed live
   for a type-19 job (the 2026-11-17 attempt returned 测验不存在 — likely the wrong
   call shape, since the H5 page calls it with `testId` and `uid`).
3. The stored type-19 answer contains `<pictures></pictures>`, which this engine
   never emits. Whether that node is server normalisation or comes from a
   different client is unresolved.
4. Whether `test/redo` accepts a type-19/20 dictation paper at all, and which
   code a real 默写 job returns. Blocked by the same missing test record: the
   existing candidates answer 测验不存在 before any redo is attempted.
5. `submitType`, `killStudyTask` and `qstJson` on the gzip task-score route stay
   unknown; they are only needed if the per-question textbook path is built.

`reTest`, `taskStatus`, `submitTime` and `disabledOnline` must never be treated
as permission to submit or resubmit — only the `test/redo` response is. Do not
try candidate write routes against a real account. A session rejected with
`-21/-22/-23` must be reported, with no second login in the same task.

## 14. Homework retest: the native reset

The report page's 重新测试 only hands `textbook.html` to the native bridge
(`type: "textbook"`, `webCumstomParams.reset`); the page calls no API. For a
submitted photo/textbook job, replaying the textbook task-score row with the
same answer and score returns `code 0` and writes nothing: `finish/job/v2`
keeps its `studyDate` and `batchId`, and `job/info.submitTime` is unchanged.

Two explanations remain:

1. the native bridge performs a reset call before opening the page — its route
   is unknown and is not `homework/test/redo` (that endpoint returns `-3` for
   these jobs, and the APK contains no `homework/test` path at all);
2. the server suppresses an identical answer and would accept genuinely new
   content, in which case the observed tablet resubmit (a new photo) needs no
   reset at all.

Resolve with one on-device capture of 重新测试 followed by 提交 (HTTP Canary /
Reqable / mitmproxy over wireless adb), or by submitting genuinely new content
to an old unsubmitted job. Do not overwrite a graded answer to test this.
