# File-backed SQLite read/write demo

Published issue: [GitHub #1](https://github.com/andrewgilliland/sqlite-workbench/issues/1).

## Problem Statement

SQLite Workbench currently displays a native shell, a database-opening placeholder, and a functioning light/dark switch. It cannot open a SQLite database, read records, or persist writes. A user needs a small, trustworthy demonstration that SQL executed through the app reaches a real local database file and remains saved after restarting—not just an updated screen or an in-memory database.

## Solution

Provide an Open Demo Database action that creates or reopens an app-owned SQLite database at a stable platform app-data location. Display its full path and the notes table. Provide a SQL editor, Run control, results table, and clear execution status. Execute a single supported statement at a time, commit successful writes immediately, and keep the UI responsive. Prove persistence through real-file tests, production-UI interactions, an application restart, and independent SQLite CLI inspection.

This spec implements the scope agreed in conversation and recorded in ADR 0001, “Build the first SQLite demo around an app-owned database.” The ADR remains marked proposed; publication of this spec does not silently change its status.

## User Stories

1. As a demo user, I want to open a sample database with one action, so that I can explore SQLite without preparing a file myself.
2. As a demo user, I want the first opening to create a notes table with sample data, so that I can immediately run a meaningful read.
3. As a returning user, I want the same database to reopen, so that my earlier changes remain available.
4. As a returning user, I want an empty notes table to stay empty after reopening, so that deleting records does not cause hidden reseeding.
5. As a user, I want database location to be independent of my launch directory, so that starting the app differently does not appear to lose my data.
6. As a user, I want to see the full active database path, so that I can inspect the exact file independently.
7. As a user, I want to see notes in the database explorer after opening, so that I know which table is available.
8. As a user, I want a clear disconnected or opening state, so that I do not mistake an incomplete operation for a usable database.
9. As a user, I want opening failures explained without resetting the file, so that inaccessible or damaged data is not silently replaced.
10. As a user, I want to enter and edit SQL, so that I can choose the read or write to demonstrate.
11. As a user, I want Run unavailable before a database is ready, so that I cannot execute against a nonexistent session.
12. As a user, I want a SELECT to show column names and rows, so that I can understand returned data.
13. As a user, I want a successful empty SELECT to show its columns and a no-rows message, so that I can distinguish it from an execution error.
14. As a user, I want cell values presented consistently, so that null values and ordinary text are not confused.
15. As a user, I want no more than 200 rows displayed, so that a large result does not overwhelm the demo.
16. As a user, I want an explicit truncation indicator when additional rows exist, so that I do not assume the displayed rows are the complete result.
17. As a user, I want to insert a note, so that I can prove the app writes to the database file.
18. As a user, I want to update a note, so that I can demonstrate changing persisted data.
19. As a user, I want to delete a note, so that I can demonstrate removal and clean up the acceptance record.
20. As a user, I want successful writes to show an affected-row count, including zero, so that I know what the statement changed.
21. As a user, I want writes committed without a separate Save action, so that reported success means the write completed and committed.
22. As a user, I want failed writes to leave no partial changes, so that errors do not silently damage the demonstration data.
23. As a user, I want multiple statements rejected before any execute, so that pasting a script cannot partially succeed.
24. As a user, I want unsupported schema, attachment, and transaction commands rejected, so that the demo's execution contract is clear.
25. As a user, I want useful SQL error messages, so that I can correct a statement and try again.
26. As a user, I want lock failures distinguished from SQL errors, so that I know when another connection is preventing execution.
27. As a user, I want slow execution interrupted with a timeout message, so that a problematic statement cannot occupy the demo indefinitely.
28. As a user, I want only one database operation executing at a time and a visible busy state, so that repeated clicks do not enqueue accidental duplicate writes.
29. As a user, I want the window to remain responsive while database work runs, so that I can interact with the app during execution.
30. As a user, I want theme changes to preserve the connection, SQL text, results, and execution state, so that changing appearance does not reset my work.
31. As a user, I want a valid statement to succeed after a rejected statement, SQL error, lock failure, or timeout, so that recovery does not require restarting.
32. As a user, I want controls and status to remain keyboard-accessible and meaningfully labeled, so that the new workflow preserves the shell's accessibility.
33. As a user, I want a written note to survive closing and restarting the app, so that persistence is demonstrated beyond a connection lifetime.
34. As a user, I want to inspect that same note with the SQLite CLI, so that verification does not depend solely on the app's own results.

## Implementation Decisions

- **Incremental delivery:** establish file persistence first, then execution restrictions and limits, then database-opening UI, read/write UI, and final native acceptance. Each slice must satisfy its verification gate before proceeding.
- **Database identity:** resolve a stable platform app-data directory; on macOS use the SQLite Workbench directory under Application Support. Choose one deterministic filename and display the full resolved path. The exact filename and Rust SQLite library remain implementation choices. Tests must supply isolated temporary locations instead of touching the user's demo data.
- **Initialization:** initialize a notes table with an integer primary-key id and text body and deterministic sample data on first creation only. Initialization must be atomic. Reopening an existing file must preserve its contents, including an empty table, and never overwrite an invalid or incompatible existing file to repair it silently.
- **Database module:** introduce one cohesive database-session interface used by the application for opening/initializing the demo and executing statements. Return structured execution outcomes and errors; keep SQLite connection handling, permission enforcement, transactions, timeouts, and conversion of results private to that module. Prefer a small public interface over testing internal helpers.
- **Statement contract:** accept exactly one SELECT, INSERT, UPDATE, or DELETE statement. A trailing terminator and comments are not additional statements. Reject empty input, additional statements, schema changes, attachments, transaction controls, and other unsupported operations before user SQL can change data. Use SQLite-aware preparation/validation rather than splitting on semicolons or trusting a first-keyword check. Legitimate quoted text or comments must not break statement counting. Internal initialization is separate from user SQL permissions.
- **Atomic write contract:** execute supported writes atomically and commit before reporting success. Statement errors, lock failures, timeouts, and commit failures must not leave a partial write or an open user-controlled transaction. Never infer affected rows from previously executed statements. Keep successful zero-row writes distinct from errors.
- **Results:** report returned column names and ordered display rows for statements that return data; report affected-row count for writes. Preserve meaningful distinctions for SQL null and ordinary scalar values. Limit displayed rows to 200; distinguish exactly 200 rows from a result containing more than 200. Results and success messages must correspond to the completed statement, not stale output from an earlier run.
- **Write results:** if a supported write returns rows, apply the display cap without abandoning statement completion or commit. Truncation is a presentation limit, not an implicit rollback or a partially applied write.
- **Execution limits:** apply a five-second execution deadline and a one-second SQLite lock wait. Database work and lock waits run off the UI thread. A timeout must interrupt the underlying SQLite work, not merely stop waiting in the UI while a write continues in the background. Distinguish unsupported input, SQLite errors, lock failures, timeout, and open failures in user-facing outcomes.
- **Application ownership:** retain session, editor, results, status, and background-task state in persistent application-owned entities. Keep header, explorer, workspace, and status presentation modular. Preserve current theme switching and window-close behavior. Dropped views and late completions must not update a released or unrelated session.
- **Opening workflow:** replace the existing Open Database placeholder with Open Demo Database. Opening creates or reuses the same file, shows a busy state, exposes the path and notes table on success, and retains a truthful disconnected/error state on failure. Avoid resetting a healthy active session on a redundant open or allowing reopening to race with execution.
- **Workspace:** provide retained SQL input and a Run control alongside returned results and execution status. Disable duplicate execution while busy and Run while disconnected. Permit editing and retry after completion or failure. A theme change must not recreate editor or connection state.
- **Restrictions:** these are demo scope controls, not a general security sandbox. No arbitrary-file picker or implicit repair/reset workflow is required.

## Testing Decisions

- **Approved seams:** Q8 in the planning conversation explicitly approved (1) the public database interface backed by real temporary files and (2) real input through the production UI. Reuse those seams; do not introduce separate tests for every extracted presentation module. Native restart plus independent CLI inspection is an additional acceptance procedure, not another internal testing interface.
- **Good tests:** assert externally observable behavior through the same interfaces production uses. Use actual SQLite files and native UI input rather than mocks of database internals, private helper assertions, or a separate imitation UI. Use independent expected values and known data. Demonstrate test sensitivity with failing tests before implementation where practical.
- **Prior art:** the existing GPUI integration test renders the production Workbench, clicks the real theme switch and opening control, and inspects native labels, checked state, and the accessible status region. Extend that pattern to retained SQL input, Run, result/error presentation, and session preservation. Existing placeholder expectations must be replaced with genuine database behavior.
- **Persistence coverage:** create and seed a fresh temporary file; insert, update, and delete known data; close the session; reopen and read it back. Reopening must neither duplicate seed data nor reseed after deleting all rows. Verify bad existing files and initialization/open failures are surfaced without overwriting existing contents.
- **Execution coverage:** support reads, empty reads, ordinary text and null display, successful writes, zero affected rows, and supported writes returning rows. Reject scripts before the first statement executes, including a valid write followed by a second statement. Cover quoted semicolons and comments, unsupported commands, invalid SQL, and a successful query after each failure.
- **Atomicity coverage:** use fixtures that exercise real statement failure after write work begins, and verify through reopening or a separate connection that no partial change committed. Apply the same persistence checks to interrupted writes and commit failures where reproducible.
- **Limit coverage:** assert behavior at fewer than 200 rows, exactly 200, and more than 200. For a write returning more than 200 rows, verify all intended changes committed despite display truncation. Exercise real lock contention using a second connection and an interruptible slow statement for timeout recovery. Avoid exact scheduler timing assertions; use bounded completion checks and observe connection recovery and data integrity.
- **UI coverage:** open a temporary demo file through the production workflow; verify path and explorer state; enter SQL and Run; inspect result headers, expected rows, empty/truncated results, and write/error statuses. Check Run is unavailable before connection and cannot submit duplicate work while busy. Verify valid execution after an error and theme switching without loss of SQL text, session, results, or status.
- **Native acceptance:** use a fresh demo file. Read initial notes; insert a note whose body is Persistence demo; update it to Persistence verified; read it back; run invalid SQL and then a valid read. Close and restart the application, reopen the same file, and verify the updated note both in-app and through the SQLite CLI. Delete the acceptance note and verify absence. Record the exact file, observed outcomes, platform, and revision; do not count this as performed merely because automated tests pass.
- **Completion gate:** formatting, typechecking, linting, the native build, the full automated test suite, and review pass. Native acceptance evidence is recorded separately. Implement and commit in small Conventional Commit slices; code review must find no blocking issues before declaring completion.

## Out of Scope

- Opening arbitrary user-selected databases or a native file picker.
- Schema editing, migrations for arbitrary schemas, table creation through user SQL, and a general schema browser.
- Multi-statement scripts, manually controlled transactions, and separate Save or transaction-management controls.
- Attaching databases, unsupported SQLite commands, remote databases, or security sandbox guarantees.
- Query history, saved queries, exports, pagination beyond the display cap, multi-session/multi-database management, and a dedicated record-editing form.
- Persisted theme preferences, live following of system appearance after startup, and other unrelated shell changes.
- Full process-crash/power-loss resilience certification, comprehensive performance benchmarking, and automated pixel-regression coverage.

## Further Notes

- The current code has no SQLite dependency or implemented database session, editor, results table, or asynchronous execution workflow. This is implementation work, not integration of an existing database feature.
- Keep domain vocabulary precise: the database file is persistent storage; a connection/session accesses it; a statement is one executable SQL instruction; a result set contains returned columns and rows. Connected, executed, and committed are different outcomes.
- The source ADR is proposed but the scope above was confirmed in conversation. Preserve its status unless explicitly changed by a separate decision; this spec does not claim an accepted ADR already exists.
- A fresh acceptance file must be prepared explicitly; the application must never reset the existing demo database merely to make an acceptance run reproducible.
- Library choice, connection ownership mechanics, deterministic seed contents, and filename should be documented during implementation while preserving these behavioral contracts. Avoid speculative abstractions for future arbitrary-database features.
