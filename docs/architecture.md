# Architecture

## Why this stack

The reference implementation of this project was a Python terminal client. It
worked, but every feature had to be re-plumbed through a menu loop, there was
no place to put a real UI, and the read-aloud and wrong-note flows were
half-finished. The rewrite therefore starts from an orphan branch with a
different shape:

- **Rust** owns everything that must be correct: HTTP, auth, XML, caching,
  submission state machines.
- **React** owns everything that must be pleasant: layout, filtering, progress.
- **Tauri 2** glues them without shipping a browser runtime.

The two halves talk over typed Tauri commands, not direct network calls, so the
webview never sees a credential.

## Layers

```
frontend (React)          renders state, sends intents
    │ invoke
src-tauri (commands)      validates input, scopes by account, emits events
    │
up366-sync                aggregation, cache policy, submission workflows
    │
up366-protocol            endpoint table, encoding, signatures, HTTP client
up366-media               audio probing, OSS upload
up366-store               SQLite + OS credential store
    │
up366-core                pure models: ids, status rules, kinds
```

Rules:

1. `up366-core` has no I/O at all. If a rule needs to be tested, it belongs
   here.
2. Only `up366-protocol` knows upstream field names. Everything above it works
   with typed models that keep the original JSON in a `raw` field for forward
   compatibility.
3. Only `up366-store` touches SQL and the credential store.
4. A command never assembles a request body; it calls a service.
5. Any write command is a two-step `prepare` / `execute(confirm_token)` pair.

## Credentials

The application persists **the account and its session state**, never the
password.

| Data | Where |
| --- | --- |
| username, display name, uid, school id, last login | SQLite `accounts` |
| TGT, cookies, client id, user type | OS credential store, one entry per account |
| plaintext password | never written |

A session that the server rejects after the service-ticket retry is marked
`needs_reauth`, its secret is removed, and the UI asks for the password again.

## Aggregation rule

The job list is one flat list. Courses are ordered by their upstream position;
within a course the upstream array order wins. Nothing is sorted by name, date
or status, because the official client does not. A job that appears under two
classes produces two rows, each labelled with its class.

See `crates/up366-sync/src/aggregate.rs`.

## Status rule

One function decides a job's state from mixed signals, in this order: finished
(finished list, `taskStatus == 1`, `stuStatus > 1`) → returned (`stuStatus == 1`)
→ expired (deadline passed) → pending → unknown. Everything that renders a
status calls it; there is no second implementation in the UI.

See `crates/up366-core/src/status.rs`.

## Write safety

Every destructive path follows the same shape:

1. Build the exact payload without sending it.
2. Show a preview: what will be sent, to which endpoint, how many items.
3. Ask for confirmation; the backend issues a single-use token with a TTL.
4. Execute once, then read the state back to verify.
5. Record the outcome per item so a partial failure can be resumed.

Batch operations (wrong-note cleanup) run serially and stop after three
consecutive failures. A timeout is recorded as `unknown` and is never retried
automatically.

## Performance

- One HTTP connection pool per account; no per-request client construction.
- List pages never fan out per row. Scores come from the bulk finished-task
  endpoint; missing rows are filled lazily when visible.
- SQLite in WAL mode, indexed by `(account_id, course_id)` and
  `(account_id, job_id)`.
- The frontend virtualises the job list and hydrates details on demand.
