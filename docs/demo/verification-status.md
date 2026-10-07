# Demo verification status

Issue: [#6](https://github.com/andrewgilliland/sqlite-workbench/issues/6).

## Automated verification — passed

Date: 2026-10-06. Platform: macOS, Apple Silicon. Tested the issue #6 changes
applied to baseline revision `6796c590cac4b30687f8731ca8c8a71f32c6f8d8`.
The native runner records the exact committed revision and dirty-checkout status
for the later manual acceptance run.

- `cargo fmt --check`: passed.
- `cargo check --locked`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: 68 passed, zero failures.
- `cargo build --locked`: passed.
- `bash -n scripts/verify-native-demo.sh`: passed.
- Missing, relative, and extra launch arguments: each exits 2 before GUI startup.
- Independent source review: no blocking implementation or runner-safety findings.
- ShellCheck: unavailable; no ShellCheck result is claimed.

Keyboard coverage uses actual key events against the production Workbench:
initial Tab focus, Space/Enter activation, keyboard opening, retained SQL editing,
Ctrl-Tab and Ctrl-Shift-Tab traversal, Cmd/Ctrl-Enter execution without a newline,
native status labels, and session/result preservation across theme changes.
Database-path coverage verifies absolute isolated paths and rejects relative ones.

## Native acceptance — passed, evidence reviewed

The user completed all seven native stages in run `sqlite-workbench-native.fVKSie`.
The report ends with `PASSED` at 2026-10-07T03:00:45Z. The observation record
contains `ACCEPTANCE_COMPLETE=yes`; both native processes exited normally with
code zero. The updated note survived restart from a different working directory
and was independently inspected with SQLite before deletion through the app.

See [the reviewed evidence](native-acceptance-fVKSie.md) for the exact revision,
platform, file, outcomes, and an explicit correction to the report's error-category
wording. Native UI observations are human-reported; CLI and process checks are
separate evidence, not inferred from automated tests. The parent spec issue and
proposed ADR status remain unchanged.
