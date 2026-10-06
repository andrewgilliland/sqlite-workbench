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

| id | body |
| --- | --- |
| 1 | Welcome to SQLite Workbench |
| 2 | Your changes stay in this local file |

A fully initialized temporary file is published without overwriting another file.
Existing files are validated, never silently reset or reseeded—even if all notes
have been deleted. Invalid, incompatible, or read-only files produce an error.
The preview shows the first 200 notes ordered by id and reports additional rows.
Reopen the app to refresh externally changed data; this slice has no live refresh.
There is no SQL editor, arbitrary-file picker, or app-driven writing yet.

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
Run individual targets with `cargo test --test database --locked`,
`cargo test --test opening --locked`, or `cargo test --test theme --locked`.
These tests verify behavior and native accessibility properties, not pixels or
packaged-app restart. Native SQL read/write/restart acceptance belongs to the final
demo ticket; this slice does not claim that workflow is complete.

## Structure

- `src/main.rs`: framework initialization, assets, window creation, and app lifecycle.
- `src/lib.rs`: shared production modules for the binary and integration tests.
- `src/database.rs`: demo path, atomic initialization, session, schema validation, preview.
- `src/shell.rs`: shell composition, session state, and retained background opening task.
- `src/shell/header.rs`: title, theme switch, and Open Demo Database button.
- `src/shell/database_explorer.rs`: database explorer sidebar.
- `src/shell/welcome_panel.rs`: welcome content.
- `src/shell/notes_panel.rs`: scrollable file-backed notes preview.
- `src/shell/status_bar.rs`: accessible connection status and framework label.

The child modules render stateless elements. The shell owns the active session and
passes the opening callback to the header. Tests import the same production library.

The application uses the styled component layer; "shell" refers to the desktop layout,
not the optional JavaScript extension system (`gpui-shell`).

## Next slices

1. Execute bounded read statements through a SQL editor (issue #3).
2. Add atomic committed writes (issue #4).
3. Display returning write results without limiting the write (issue #5).
4. Verify native restart and independent SQLite inspection (issue #6).

Startup follows the [GPUI Kit getting-started example](https://gpui-kit.com/docs/getting-started).
GPUI Kit and its source examples are licensed under Apache-2.0.
