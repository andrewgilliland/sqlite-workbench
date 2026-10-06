---
status: proposed
date: 2026-10-05
---

# Build the first SQLite demo around an app-owned database

The workbench currently has a native shell but no database operations. The first
demo will create and reopen an app-owned SQLite database, execute individual SQL
reads and writes, and prove persistence across restart and independent inspection.
This deliberately prioritizes a small, verifiable end-to-end workflow over opening
arbitrary databases or building a full SQL workbench.

## Decision

- Store the demo database in a stable platform app-data location, independent of
  the working directory. On macOS, use the SQLite Workbench directory under
  Application Support. Display the full database path in the app.
- Provide an **Open Demo Database** action that creates the database on first use
  and reopens it afterward. Initialize a `notes` table with `id` and `body`, seeding
  only on initial creation. Never silently reset or reseed an existing database.
- Provide a SQL editor, Run control, results table, and execution status. The
  explorer lists `notes` after opening the database.
- Execute one statement per run. Allow `SELECT`, `INSERT`, `UPDATE`, and `DELETE`
  against the demo database. Reject multiple statements, schema changes,
  attachments, and transaction-control statements before they can change data.
- Commit successful writes immediately; there is no separate Save action.
  Report write success and affected-row count only after completion and commit.
  Failed writes must not leave partial changes.
- Run database work off the UI thread and allow only one execution at a time.
  Use a five-second execution timeout and a one-second lock wait. Present SQL
  errors, timeouts, and lock failures distinctly, and permit subsequent execution
  after an error.
- Display at most 200 result rows with an explicit truncation indicator. The
  display limit must not cancel a write or cause it to apply only partially.

## Considered options

- **Arbitrary existing databases:** closer to the eventual product, but introduces
  file selection, unknown schemas, permissions, and risk to user data before the
  persistence workflow is proven. Defer it.
- **A record-editing form:** simpler input, but adds a domain-specific UI that does
  not advance the SQL workbench. Prefer a small SQL editor.
- **Scripts and explicit transactions:** useful later, but require a broader
  execution and transaction-lifecycle contract. Start with atomic, individually
  committed statements.
- **UI-only verification:** faster, but an updated result can hide missing file
  persistence. Require reopen tests and independent SQLite inspection.

## Consequences

The demo is intentionally narrower than a general-purpose database client. SQL
restrictions are scope controls, not a claim of security sandboxing. Database
contents persist, while theme choices remain session-only. Query history, schema
editing, arbitrary-file selection, scripts, and user-managed transactions are out
of scope. The SQLite Rust library and exact database filename are implementation
details, not selected by this decision.

## Verification gates

Implement in small slices, verifying each before moving to the next:

1. **File persistence:** test the public database interface using real temporary
   files: create, seed, write, close, reopen, and read. Reopening must not reseed,
   including after all notes have been deleted.
2. **Execution contract:** test single-statement restrictions, read/write results,
   atomic failures, error recovery, lock waits, timeouts, and result truncation.
3. **Open workflow:** verify the production UI opens the stable database, shows its
   path and `notes`, and reports failures without claiming a connection succeeded.
4. **SQL workflow:** enter SQL and activate Run through real UI input; verify
   columns, rows, empty results, truncation, affected-row counts, and errors. Theme
   switching must preserve the active session and its state.
5. **Independent acceptance:** run each statement below separately, read back the
   change, and verify recovery from invalid SQL. Close and restart the app, reopen
   the same database, and confirm the updated record both in-app and with the
   SQLite CLI. Finally delete the demo record and verify its absence.

```sql
SELECT id, body FROM notes ORDER BY id;
INSERT INTO notes (body) VALUES ('Persistence demo');
UPDATE notes SET body = 'Persistence verified' WHERE body = 'Persistence demo';
SELECT id, body FROM notes WHERE body = 'Persistence verified';
```

Use a fresh demo file for the acceptance sequence to avoid confusing earlier runs
with newly persisted data. Formatting, typechecking, linting, the build, the full
test suite, and review must pass before completion. Native restart and CLI checks
must be recorded separately from automated UI tests; neither substitutes for the
other.
