# SQLite Workbench

A small native desktop starter built with [GPUI Kit](https://gpui-kit.com/).

## Requirements

- Rust 1.95.0 with Cargo, selected automatically by `rust-toolchain.toml` when
  using rustup. Although GPUI Kit's installation guide lists Rust 1.92, its
  published GPUI dependency uses `std::hint::cold_path`, stabilized in 1.95.
- On macOS: macOS 15 or newer and Xcode Command Line Tools. GPUI's Metal
  shader build also needs a working Metal compiler in the selected Xcode installation.
- For other platforms, follow the [GPUI Kit installation guide](https://gpui-kit.com/docs/installation).

## Run

From this directory:

```sh
cargo run --locked
```

The first build downloads and compiles the UI framework and can take several minutes.
Click **Open Demo Database** to create or reopen the app-owned SQLite file.
The explorer shows the full path and `notes` table; the main panel previews notes
read from disk. Opening and preview reading run in the background, with visible
opening, connected, and error states. Duplicate opening is disabled while opening
or connected; errors allow retry. Closing the last window exits the application.

## Demo database

The app uses `rusqlite` 0.38 with bundled SQLite, so a separate SQLite installation
is not needed to run it. `dirs` resolves platform app data independently of the
launch directory. On macOS the file is:

`~/Library/Application Support/SQLite Workbench/demo.sqlite3`

On other platforms it is `SQLite Workbench/demo.sqlite3` under the platform's
local app-data directory. The UI shows the full resolved path.

First creation atomically seeds `notes (id INTEGER PRIMARY KEY, body TEXT)`:

| id  | body                                 |
| --- | ------------------------------------ |
| 1   | Welcome to SQLite Workbench          |
| 2   | Your changes stay in this local file |

A fully initialized temporary file is published without overwriting another file.
Existing files are validated, never silently reset or reseeded—even if all notes
have been deleted. Invalid, incompatible, or read-only files produce an error.
The preview shows the first 200 notes ordered by id and reports additional rows.
The initial preview is a snapshot; use a query to read externally changed data.
There is no arbitrary-file picker; all reads and writes use this demo database.

## Run SQL

After opening the demo, edit the SQL and press **Run**. The editor starts with:

```sql
SELECT id, body FROM notes ORDER BY id;
```

Run one SELECT, INSERT, UPDATE, or DELETE, including common table expressions.
Comments, a trailing terminator, and quoted semicolons are accepted. Writes may
include RETURNING. Scripts, schema changes, PRAGMAs, attachments, transaction controls,
EXPLAIN, and top-level VALUES are rejected before execution. SQLite's parser
validates the entire statement tail before any write; an authorizer permits only
supported actions. Writes target the demo's `notes` table, not arbitrary tables.
These are scope restrictions, not a security sandbox.

Writes commit immediately, with no Save action. The app displays **Write committed:
N rows affected** only after COMMIT succeeds; zero affected rows is valid. Counts
exclude changes made by triggers. A new run clears the prior outcome, so failed
writes never reuse an earlier successful count. Read changes back with a SELECT.

RETURNING writes show their columns and typed rows alongside the full affected-row
count after commit. Empty returned results retain their headers. Only the first
200 rows are copied for display, but the entire statement is stepped to completion
and committed—even when more than 200 rows are returned. Truncation limits the
display, never the number of persisted changes. Result-copy errors roll back the
write; no collected rows are presented as committed success on a failed write.

Each write owns an internal transaction. Constraint failures (including partial
`OR FAIL` execution), lock failures, interrupted writes, and commit failures roll
back before the worker returns. Deadline callbacks are disabled during rollback
so cleanup is not interrupted. Successful COMMIT is the point of no return: a
completed commit is reported as success, never as a post-commit timeout. Exceptional
rollback failures caused by storage/allocation faults are not covered by the tests.

Results show column headers even when empty. Text is quoted and escaped, SQL null
is shown as `(SQL NULL)`, and blobs use hexadecimal `X'…'` notation. At most 200
rows are displayed; additional rows produce an explicit truncation message.

Queries run off the UI thread, with one operation at a time. Run is disabled before
connection and while busy; theme changes preserve SQL, results, and session state.
New runs clear old results. Unsupported SQL, query errors, locks, and timeouts have
distinct messages and allow retry without reconnecting.

The connection waits up to one second for SQLite locks. Execution has a five-second
cooperative deadline spanning preparation, stepping, and write commit: callbacks interrupt
the SQLite VM, rather than just abandoning the UI wait. A long built-in function or
blocking I/O may delay the next callback; this is not a hard process-level timeout.
Unknown or side-effecting functions and PRAGMA virtual tables are denied. Supported
functions include common aggregates, string/date operations, and window functions.

To bound copied results, SQL is capped at 16 KiB, columns at 256, each text/blob/name
payload at 1 MiB, and total copied payload at 8 MiB. Limit violations are reported
as unsupported; cells are never silently truncated. These caps do not bound all
SQLite intermediate allocations. Invalid UTF-8 text produces a query error.

Use the header's theme switch to change the entire app's theme:
on selects **Dark mode**, off selects **Light mode**, and the label shows the
current mode. Startup matches the system
appearance; manual changes last for the current session only.

## Development checks

```sh
cargo fmt --check
cargo check --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Database tests use isolated real temporary files, not your demo data. They cover
seed-once initialization, reopen persistence, external edits, empty tables,
concurrent creation, permissions, invalid files, and preview limits. UI tests use
the production Workbench with temporary paths and real clicks to verify opening,
preview content, failure/retry, duplicate-opening prevention, and theme preservation.
Read tests cover SQLite types, statement restrictions, row and payload limits,
real exclusive locking, five-second VM interruption, and subsequent recovery.
Query UI tests enter SQL through native input, verify results/error presentation,
prevent duplicate runs, and change themes while a real query is running.
Write tests cover CRUD, zero counts, reopened persistence, script
rejection, partial constraint failures, interrupted inserts, and actual lock
failures during execution and commit. Write UI tests enter SQL through native
input and verify committed counts, recovery, duplicate prevention, and persistence.
RETURNING tests verify typed/empty results, row-cap boundaries, persistence of
changes beyond the display cap, result-copy failure rollback, timeout and commit
failure recovery, and production-UI theme/session preservation.
Run individual targets with `cargo test --test database --locked`,
`cargo test --test opening --locked`, `cargo test --test theme --locked`,
`cargo test --test reads --locked`, `cargo test --test query --locked`,
`cargo test --test writes --locked`, `cargo test --test write_ui --locked`,
`cargo test --test returning --locked`, or `cargo test --test returning_ui --locked`.
These tests verify behavior and native accessibility properties, not pixels or
packaged-app restart. Native SQL read/write/restart acceptance belongs to the final
demo ticket; this slice does not claim that workflow is complete.

## Structure

- `src/main.rs`: framework initialization, assets, window creation, and app lifecycle.
- `src/lib.rs`: shared production modules for the binary and integration tests.
- `src/database.rs`: demo path, atomic initialization, session, schema validation, preview.
- `src/database/read.rs`: guarded SELECT execution, typed results, limits, and errors.
- `src/database/write.rs`: single-statement dispatch and atomic committed writes.
- `src/shell.rs`: shell composition, retained editor/session, and background tasks.
- `src/shell/header.rs`: title, theme switch, and Open Demo Database button.
- `src/shell/database_explorer.rs`: database explorer sidebar.
- `src/shell/welcome_panel.rs`: welcome content.
- `src/shell/notes_panel.rs`: scrollable file-backed notes preview.
- `src/shell/query_panel.rs`: retained SQL editor and Run control.
- `src/shell/query_results.rs`: scrollable typed query results and summaries.
- `src/shell/status_bar.rs`: accessible connection status and framework label.

The child modules render stateless elements. The shell owns the active session and
passes the opening callback to the header. Tests import the same production library.

The application uses the styled component layer; "shell" refers to the desktop layout,
not the optional JavaScript extension system (`gpui-shell`).

## Next slices

1. Verify native restart and independent SQLite inspection (issue #6).

Startup follows the [GPUI Kit getting-started example](https://gpui-kit.com/docs/getting-started).
GPUI Kit and its source examples are licensed under Apache-2.0.
