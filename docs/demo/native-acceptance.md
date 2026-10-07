# Native demo acceptance

Issue: [#6](https://github.com/andrewgilliland/sqlite-workbench/issues/6).

## Evidence status

**Native acceptance is pending.** Automated checks and source review do not prove
that a human opened, restarted, or operated the real desktop app. This document is
the repeatable procedure, not a claim that its manual steps have passed.

The runner produces a separate report for each run. Share that report before
closing issue #6. The parent spec issue and ADR status remain unchanged.

## Run the guide

From the repository root:

```sh
./scripts/verify-native-demo.sh
```

Requirements: a graphical desktop session, Rust/Cargo, Git, and the SQLite CLI
with `-readonly` support. The guide builds the current checkout before launch.
On macOS, `/usr/bin/sqlite3` is normally available. No credentials are collected.

Every invocation creates a unique directory in the operating-system temporary
directory. It retains the SQLite database, report, observations, and launch/build
logs on success or failure. It never deletes or resets a previous run or the
ordinary application-data database. Do not remove the run directory until its
evidence is recorded.

The native binary supports `--demo-data-dir ABSOLUTE_DIRECTORY` specifically to
select an explicit demo location for reproducible runs. It uses `demo.sqlite3`
inside that directory, still with normal seed-once/no-overwrite semantics.
Relative directories and unexpected arguments are rejected before GUI startup.
Without this option, normal platform app-data behavior is unchanged. This is not
an arbitrary-database file picker or a reset feature.

## Seven stages

1. **Fresh native opening:** the runner builds and launches from the repository
   directory. Use keyboard controls to open the demo. Verify the exact displayed
   path, `notes` table, and two seeded notes. The CLI independently reads them.
2. **Read/write/read:** execute each statement separately using the editor and Run.
   Read initial notes; insert `Persistence demo`; update it to `Persistence verified`;
   read it back. Expect one affected row for each write. CLI checks verify each step.
3. **Recovery and accessibility:** run invalid SQL, then a valid read. Check focus,
   editor/Run/theme/status labels, both themes, and retained session/results. Run a
   slow recursive SELECT, observe responsive theme/editing and unavailable Run,
   then verify recovery after the cooperative timeout.
4. **Close:** close the first window normally. The runner waits for its actual
   process to exit with code zero; forced termination cannot pass this stage.
5. **Different-directory restart:** launch the same binary from the new temporary
   directory using the same absolute data option. Reopen and read the updated note.
   CLI inspection confirms the exact same file still contains it.
6. **Delete:** delete only the acceptance note through the app; read an empty result
   with headers. CLI checks verify absence and that the seed notes are unchanged.
7. **Final exit and evidence:** close normally again. Review the generated report
   and add any public observations. Only explicit confirmations plus all CLI and
   process checks produce the final `PASSED` section.

Use the statements printed by the guide. The guide never performs writes through
the CLI: all acceptance inserts, updates, and deletes must happen through the app.
Unknown observations, EOF, refusal, or Ctrl-C leave the run incomplete. Abort
cleanup may terminate the runner-owned process; that is expressly not normal-exit
evidence. Logs and the file remain available for diagnosis.

## Keyboard controls

- Tab / Shift-Tab navigate outside the SQL editor.
- Ctrl-Tab / Ctrl-Shift-Tab navigate forward/backward through enabled controls,
  including leaving the editor. Tab inside the editor remains indentation.
- Enter or Space activates a focused button or theme switch.
- Cmd-Enter on macOS, Ctrl-Enter elsewhere, runs the current SQL from the editor
  without inserting a newline. Disconnected/busy checks still apply.
- Cmd-A on macOS, Ctrl-A elsewhere, selects all editor text for replacement.
- Opening moves focus to the editor before the open control is disabled. Run
  returns focus to the editor; themes do not recreate the input or active session.

## What the report proves

The report records revision, dirty-checkout status, platform, SQLite CLI version,
full database path, exact launch working directories and commands, CLI output,
and observed process exits. Native visual/focus observations are explicitly
**human-reported**, not tool-observed or implied by automated test results.

Reports do not contain secrets; they do contain local paths and public observations.
Review before sharing. A report ending `INCOMPLETE / ABORTED` is not acceptance.
Repeat with a fresh run rather than editing a failed report into a passing one.

Automated coverage uses real temporary files and production-UI input, including
keyboard navigation, retained editor state, atomic writes, returning-result limits,
locks, deadlines, and recovery. It is separate from this native restart/CLI evidence.
The deadline remains cooperative, not a hard process-level limit; exceptional
storage-fault rollback cleanup remains outside fault-injection coverage.
