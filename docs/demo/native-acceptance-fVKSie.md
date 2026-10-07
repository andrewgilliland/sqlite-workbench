# Reviewed native acceptance: fVKSie

Issue: [#6](https://github.com/andrewgilliland/sqlite-workbench/issues/6).

## Provenance

- Run started: 2026-10-07T02:52:13Z; completed: 2026-10-07T03:00:45Z.
- Report result: `PASSED`; observations contain `ACCEPTANCE_COMPLETE=yes`.
- Tested application revision: `91f97c11cf92d88217378c2ca4860ffdf771cd59`.
- Dirty files recorded at launch: this verification-status document and the
  native guide script. No application-source changes were recorded at launch.
- Platform: macOS 26.6.2, build 25G83, Apple Silicon arm64.
- SQLite CLI: 3.51.0, using read-only connections.
- Database: `/private/var/folders/8c/pclypcmd2kd7n73tz7mbv8140000gn/T/sqlite-workbench-native.fVKSie/demo.sqlite3`.
- Original report: the same directory's `acceptance.md`; observations:
  `observations.env`; build log: `build.log`; launch logs: `launch-1.log` and
  `launch-2.log`. Preserve these original artifacts; this document is a reviewed
  summary, not a replacement or alteration of the source report.

## Human-reported native observations

The user operated the native app and confirmed fresh isolated opening, initial
notes, separate read/insert/update/read execution, invalid-input rejection and
successful recovery, keyboard access, both themes, retained editor/session/results,
responsive busy state, timeout recovery, reopening, deletion, and normal closure.
These are user observations, not assistant-observed pixels or automated UI evidence.

## Independently recorded CLI and process outcomes

1. Fresh seed contents were exactly IDs 1 and 2 with the documented bodies.
2. App INSERT produced one `Persistence demo` record.
3. App UPDATE produced one `Persistence verified` record and zero old-body records.
4. CLI readback was `3|Persistence verified`, including after the first process exit.
5. Launch 1 used working directory `/Users/andrewgilliland/Code/rust/sqlite-workbench`.
   PID 48500 exited with code zero through `wait`, without a forced kill.
6. Launch 2 used working directory
   `/private/var/folders/8c/pclypcmd2kd7n73tz7mbv8140000gn/T/sqlite-workbench-native.fVKSie`.
   The same absolute data directory reopened the same file. Native readback and
   independent CLI inspection confirmed the updated record survived restart.
7. App DELETE removed only the acceptance note. CLI found zero matching records,
   with seed IDs 1 and 2 unchanged.
8. PID 48794 exited with code zero through `wait`, without a forced kill.
   Final CLI contents were exactly:

```text
1|Welcome to SQLite Workbench
2|Your changes stay in this local file
```

Both launch logs were empty. A later independent read-only CLI recheck confirmed
these same final contents. It verifies the final state, not historical write steps;
those are supported by the original report's recorded checks.

## Error-category qualification

The guide used `SELEC id FROM notes;` but its confirmation incorrectly named
`Query error`. That command is rejected as `Unsupported SQL`. The user explicitly
confirmed seeing that error in the bottom status bar in the follow-up conversation.
The results panel's `No query results` indicates cleared results, not successful
execution. The report's truncated additional note about that display is retained
in the original artifact, not rewritten here.

Acceptance requires useful invalid-input feedback and subsequent valid execution,
not a particular category for an unrecognized operation. Recovery was confirmed.
Future guide runs use `SELECT FROM notes;` to exercise an actual SQLite syntax error
and match the `Query error` confirmation. No native rerun of that revised example
is claimed by this report.

## Other verification and limits

The implementation's automated gates previously passed all 68 tests, formatting,
typechecking, strict Clippy, native build, and independent source review. The guide
was syntax-checked and its missing-file recheck/refusal behavior tested separately.
This native report does not replace those automated gates.

The deadline remains cooperative SQLite interruption, not a hard process-level
limit. Exceptional storage/allocation-fault rollback errors are outside tested
fault-injection coverage. Parent issue #1 and ADR 0001's proposed status are unchanged.
