use sqlite_workbench::database::{CellValue, DemoSession, ExecutionResult, ReadError, ReadResult};

fn returning(session: &mut DemoSession, sql: &str) -> (usize, ReadResult) {
    match session.execute(sql).unwrap() {
        ExecutionResult::Write {
            affected_rows,
            returned: Some(result),
        } => (affected_rows, result),
        outcome => panic!("expected committed RETURNING data: {outcome:?}"),
    }
}

#[test]
fn real_recursive_returning_write_times_out_without_effects_and_cleans_hooks() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    let started = Instant::now();
    let outcome = session.execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000000000) INSERT INTO notes(body) SELECT 'deadline' FROM n RETURNING id");
    let elapsed = started.elapsed();
    assert!(matches!(outcome, Err(ReadError::Timeout)), "{outcome:?}");
    assert!(
        elapsed >= Duration::from_secs(5),
        "did not use production deadline: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "deadline overshoot: {elapsed:?}"
    );
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT id, body FROM notes ORDER BY id")
            .unwrap(),
        before
    );
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute_batch("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1200) INSERT INTO notes(body) SELECT 'external' FROM n").unwrap();
    // Preview does enough VM work to expose an expired hook before read/execute
    // can install a replacement. The external writer also proves lock release.
    let preview = session.preview().unwrap();
    assert_eq!(preview.notes.len(), 200);
    assert!(preview.truncated);
    assert_eq!(
        session
            .read("SELECT id, body FROM notes WHERE id<=2 ORDER BY id")
            .unwrap(),
        before
    );
    let (count, result) = returning(
        &mut session,
        "INSERT INTO notes(body) VALUES ('recovered') RETURNING body",
    );
    assert_eq!(count, 1);
    assert_eq!(result.rows, vec![vec![CellValue::Text("recovered".into())]]);
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT count(*), count(CASE WHEN body='deadline' THEN 1 END) FROM notes")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(1203), CellValue::Integer(0)]]
    );
}

#[test]
fn commit_lock_after_gathering_returning_rows_is_an_error_and_rolls_back() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    // SHARED permits the write and RETURNING cursor to finish, but prevents
    // COMMIT from obtaining EXCLUSIVE in rollback-journal mode.
    external
        .execute_batch("PRAGMA journal_mode=DELETE; BEGIN DEFERRED; SELECT * FROM notes;")
        .unwrap();
    let started = Instant::now();
    let outcome =
        session.execute("INSERT INTO notes(body) VALUES ('must rollback') RETURNING id, body");
    let elapsed = started.elapsed();
    external.execute_batch("ROLLBACK").unwrap();
    assert!(matches!(outcome, Err(ReadError::Locked)), "{outcome:?}");
    assert!(
        elapsed >= Duration::from_millis(900),
        "lock wait too short: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "lock wait unbounded: {elapsed:?}"
    );
    assert_eq!(
        session
            .read("SELECT id, body FROM notes ORDER BY id")
            .unwrap(),
        before
    );
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT id, body FROM notes ORDER BY id")
            .unwrap(),
        before
    );
    external.execute("UPDATE notes SET body=body", []).unwrap();
    let (count, result) = returning(
        &mut session,
        "INSERT INTO notes(body) VALUES ('recovered') RETURNING id, body",
    );
    assert_eq!(count, 1);
    assert_eq!(
        result.rows,
        vec![vec![
            CellValue::Integer(3),
            CellValue::Text("recovered".into())
        ]]
    );
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT body FROM notes WHERE id=3")
            .unwrap()
            .rows,
        vec![vec![CellValue::Text("recovered".into())]]
    );
}

#[test]
fn returning_constraint_failures_rollback_partial_or_fail_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute_batch("DELETE FROM notes; INSERT INTO notes VALUES (1, 'first'), (2, 'second'); CREATE UNIQUE INDEX unique_body ON notes(body);").unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    for sql in [
        "UPDATE OR FAIL notes SET body='collision' RETURNING id, body",
        "INSERT OR FAIL INTO notes VALUES (3, 'third'), (4, 'first') RETURNING id, body",
        "INSERT OR ROLLBACK INTO notes VALUES (3, 'third'), (4, 'first') RETURNING id",
    ] {
        let outcome = session.execute(sql);
        assert!(
            matches!(outcome, Err(ReadError::Sql(ref error)) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation)),
            "{sql}: {outcome:?}"
        );
        assert_eq!(
            session
                .read("SELECT id, body FROM notes ORDER BY id")
                .unwrap(),
            before
        );
        assert_eq!(
            DemoSession::open(&path)
                .unwrap()
                .read("SELECT id, body FROM notes ORDER BY id")
                .unwrap(),
            before
        );
        external.execute("UPDATE notes SET body=body", []).unwrap();
        assert_eq!(
            returning(
                &mut session,
                "UPDATE notes SET body=body WHERE 0 RETURNING id"
            )
            .0,
            0
        );
    }
    let (count, result) = returning(
        &mut session,
        "INSERT OR IGNORE INTO notes VALUES (3, 'third'), (4, 'first'), (5, 'fifth') RETURNING id",
    );
    assert_eq!(count, 2);
    let mut ids = result
        .rows
        .iter()
        .map(|row| match row[0] {
            CellValue::Integer(id) => id,
            _ => panic!("expected integer id"),
        })
        .collect::<Vec<_>>();
    ids.sort_unstable();
    assert_eq!(ids, [3, 5]);
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT count(*) FROM notes")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(4)]]
    );
}

#[test]
fn returning_scripts_are_validated_before_the_first_write_can_step() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    for (sql, unsupported) in [
        (
            "INSERT INTO notes(body) VALUES ('bad') RETURNING id; SELECT 1",
            true,
        ),
        (
            "UPDATE notes SET body='bad' RETURNING body; DELETE FROM notes RETURNING id",
            true,
        ),
        ("DELETE FROM notes RETURNING id; BEGIN", true),
        (
            "WITH n AS (SELECT 1) UPDATE notes SET body='bad' RETURNING id; SELECT 2",
            true,
        ),
        (
            "INSERT INTO notes(body) VALUES ('bad') RETURNING id; this is invalid",
            false,
        ),
        ("DELETE FROM notes RETURNING id; SELECT FROM notes", false),
        (
            "INSERT INTO notes(body) VALUES ('bad') RETURNING load_extension('missing')",
            true,
        ),
    ] {
        let outcome = session.execute(sql);
        if unsupported {
            assert!(
                matches!(outcome, Err(ReadError::Unsupported(_))),
                "{sql}: {outcome:?}"
            );
        } else {
            assert!(
                matches!(outcome, Err(ReadError::Sql(_))),
                "{sql}: {outcome:?}"
            );
        }
        assert_eq!(
            session
                .read("SELECT id, body FROM notes ORDER BY id")
                .unwrap(),
            before
        );
        assert_eq!(
            returning(
                &mut session,
                "UPDATE notes SET body=body WHERE 0 RETURNING id"
            )
            .0,
            0
        );
    }
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT id, body FROM notes ORDER BY id")
            .unwrap(),
        before
    );
}

#[test]
fn returning_accepts_exact_copy_limits_and_does_not_charge_discarded_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let (count, result) = returning(
        &mut session,
        &format!(
            "UPDATE notes SET body=body WHERE id=1 RETURNING {}",
            vec!["NULL"; 256].join(",")
        ),
    );
    assert_eq!(count, 1);
    assert_eq!(result.columns.len(), 256);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0].len(), 256);
    assert!(result.rows[0].iter().all(|cell| *cell == CellValue::Null));
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute("DELETE FROM notes", []).unwrap();
    // Eight cells plus the four-byte column name exactly fill 8 MiB;
    // seven individual cells also exercise the exact 1 MiB cell limit.
    for id in 1..=8 {
        let body = "x".repeat(1024 * 1024 - if id == 8 { 4 } else { 0 });
        external
            .execute(
                "INSERT INTO notes VALUES (?1, ?2)",
                rusqlite::params![id, body],
            )
            .unwrap();
    }
    let (count, result) = returning(&mut session, "UPDATE notes SET id=id+10 RETURNING body");
    assert_eq!(count, 8);
    assert_eq!(result.rows.len(), 8);
    assert!(!result.truncated);
    let mut sizes = result
        .rows
        .iter()
        .map(|row| {
            let CellValue::Text(body) = &row[0] else {
                panic!("expected text");
            };
            assert!(body.bytes().all(|byte| byte == b'x'));
            body.len()
        })
        .collect::<Vec<_>>();
    sizes.sort_unstable();
    assert_eq!(
        sizes,
        [
            1048572, 1048576, 1048576, 1048576, 1048576, 1048576, 1048576, 1048576
        ]
    );
    external.execute("DELETE FROM notes", []).unwrap();
    let body = "y".repeat(41900);
    let transaction = external.unchecked_transaction().unwrap();
    for id in 1..=201 {
        transaction
            .execute(
                "INSERT INTO notes VALUES (?1, ?2)",
                rusqlite::params![id, body],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    // Any 200 rows fit, while copying all 201 would exceed 8 MiB.
    // This does not rely on SQLite's unspecified RETURNING row order.
    let (count, result) = returning(&mut session, "UPDATE notes SET id=id+1000 RETURNING body");
    assert_eq!(count, 201);
    assert_eq!(result.rows.len(), 200);
    assert!(result.truncated);
    assert!(
        result
            .rows
            .iter()
            .all(|row| row == &[CellValue::Text(body.clone())])
    );
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT count(*) FROM notes WHERE id>1000")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(201)]]
    );
}

#[test]
fn returning_copy_errors_rollback_all_changes_and_allow_recovery() {
    for failure in ["cell", "payload", "columns", "utf8"] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("demo.sqlite3");
        let mut session = DemoSession::open(&path).unwrap();
        let external = rusqlite::Connection::open(&path).unwrap();
        external.execute("DELETE FROM notes", []).unwrap();
        external
            .execute("INSERT INTO notes VALUES (1, 'small')", [])
            .unwrap();
        let (size, additional) = match failure {
            "cell" => (1024 * 1024 + 1, 1),
            "payload" => (1024 * 1024, 9),
            _ => (1, 1),
        };
        let body = "x".repeat(size);
        for id in 2..=additional + 1 {
            external
                .execute(
                    "INSERT INTO notes VALUES (?1, ?2)",
                    rusqlite::params![id, body],
                )
                .unwrap();
        }
        let before = session
            .read("SELECT id, length(body) FROM notes ORDER BY id")
            .unwrap();
        let projection = match failure {
            "columns" => vec!["NULL"; 257].join(","),
            "utf8" => "CAST(x'ff' AS TEXT)".into(),
            _ => "body".into(),
        };
        let outcome = session.execute(&format!(
            "UPDATE notes SET id=id+100 RETURNING {projection}"
        ));
        if failure == "utf8" {
            assert!(
                matches!(outcome, Err(ReadError::Sql(rusqlite::Error::Utf8Error(_)))),
                "{outcome:?}"
            );
        } else {
            assert!(
                matches!(outcome, Err(ReadError::Unsupported(_))),
                "{failure}: {outcome:?}"
            );
        }
        assert_eq!(
            session
                .read("SELECT id, length(body) FROM notes ORDER BY id")
                .unwrap(),
            before
        );
        assert_eq!(
            DemoSession::open(&path)
                .unwrap()
                .read("SELECT id, length(body) FROM notes ORDER BY id")
                .unwrap(),
            before
        );
        // An independent writer proves rollback released its transaction.
        external
            .execute("UPDATE notes SET body=body WHERE id=1", [])
            .unwrap();
        let (count, result) = returning(
            &mut session,
            "UPDATE notes SET body='recovered' WHERE id=1 RETURNING body",
        );
        assert_eq!(count, 1);
        assert_eq!(result.rows, vec![vec![CellValue::Text("recovered".into())]]);
    }
}

#[test]
fn display_boundaries_drain_and_commit_every_crud_change_on_reopen() {
    for total in [199, 200, 201, 1201] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("demo.sqlite3");
        let mut session = DemoSession::open(&path).unwrap();
        session.execute("DELETE FROM notes").unwrap();
        for (sql, persisted_count, persisted_body) in [
            (
                format!(
                    "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<{total}) INSERT INTO notes(id, body) SELECT x, 'inserted' FROM n RETURNING id, body"
                ),
                total,
                "inserted",
            ),
            (
                "UPDATE notes SET body='updated' RETURNING id, body".into(),
                total,
                "updated",
            ),
            ("DELETE FROM notes RETURNING id, body".into(), 0, "updated"),
        ] {
            let (affected, result) = returning(&mut session, &sql);
            assert_eq!(affected, total);
            assert_eq!(result.columns, ["id", "body"]);
            assert_eq!(result.rows.len(), total.min(200));
            assert_eq!(result.truncated, total > 200);
            let expected_body = if sql.starts_with("WITH") {
                "inserted"
            } else {
                "updated"
            };
            let mut ids = result
                .rows
                .iter()
                .map(|row| {
                    assert_eq!(row[1], CellValue::Text(expected_body.into()));
                    let CellValue::Integer(id) = row[0] else {
                        panic!("expected integer id");
                    };
                    assert!((1..=total as i64).contains(&id));
                    id
                })
                .collect::<Vec<_>>();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), total.min(200));
            if total <= 200 {
                assert_eq!(ids, (1..=total as i64).collect::<Vec<_>>());
            }
            drop(session);
            session = DemoSession::open(&path).unwrap();
            assert_eq!(session.read(&format!("SELECT count(*), count(CASE WHEN body='{persisted_body}' THEN 1 END) FROM notes")).unwrap().rows,
                vec![vec![CellValue::Integer(persisted_count as i64), CellValue::Integer(persisted_count as i64)]]);
        }
    }
}

#[test]
fn zero_row_returning_keeps_headers_without_reusing_previous_rows_or_counts() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    assert_eq!(
        returning(&mut session, "UPDATE notes SET body=body RETURNING id").0,
        2
    );
    for sql in [
        "INSERT INTO notes(body) SELECT 'absent' WHERE 0 RETURNING body AS empty, NULL AS missing",
        "UPDATE notes SET body='absent' WHERE id=999 RETURNING body AS empty, NULL AS missing",
        "DELETE FROM notes WHERE id=999 RETURNING body AS empty, NULL AS missing",
    ] {
        let (count, result) = returning(&mut session, sql);
        assert_eq!(count, 0, "{sql}");
        assert_eq!(
            result,
            ReadResult {
                columns: vec!["empty".into(), "missing".into()],
                rows: vec![],
                truncated: false
            }
        );
    }
}

#[test]
fn insert_returning_commits_and_returns_the_inserted_row() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let outcome = session
        .execute("INSERT INTO notes(body) VALUES ('returned') RETURNING id, body")
        .expect("INSERT RETURNING must commit successfully");
    assert_eq!(
        outcome,
        ExecutionResult::Write {
            affected_rows: 1,
            returned: Some(ReadResult {
                columns: vec!["id".into(), "body".into()],
                rows: vec![vec![
                    CellValue::Integer(3),
                    CellValue::Text("returned".into())
                ]],
                truncated: false,
            }),
        }
    );
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT id, body FROM notes WHERE body='returned'")
            .unwrap()
            .rows,
        vec![vec![
            CellValue::Integer(3),
            CellValue::Text("returned".into())
        ]]
    );
}

#[test]
fn cte_crud_returning_preserves_names_types_and_affected_counts() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let (count, result) = returning(
        &mut session,
        "WITH \"select\"(v) AS (VALUES ('it''s;fine')) INSERT INTO main.\"notes\"(body) SELECT v FROM \"select\" RETURNING id, body, NULL AS missing, 42 AS whole, 1.25 AS fraction, x'00ff' AS bytes; -- tail ;",
    );
    assert_eq!(count, 1);
    assert_eq!(
        result.columns,
        ["id", "body", "missing", "whole", "fraction", "bytes"]
    );
    assert_eq!(
        result.rows,
        vec![vec![
            CellValue::Integer(3),
            CellValue::Text("it's;fine".into()),
            CellValue::Null,
            CellValue::Integer(42),
            CellValue::Real(1.25),
            CellValue::Blob(vec![0, 255])
        ]]
    );
    assert!(!result.truncated);
    let (count, result) = returning(
        &mut session,
        "WITH n AS (SELECT 3 AS id) UPDATE notes SET body=upper(body) WHERE id=(SELECT id FROM n) RETURNING body AS updated",
    );
    assert_eq!(count, 1);
    assert_eq!(result.columns, ["updated"]);
    assert_eq!(result.rows, vec![vec![CellValue::Text("IT'S;FINE".into())]]);
    let (count, result) = returning(
        &mut session,
        "WITH n AS (SELECT 3 AS id) DELETE FROM notes WHERE id=(SELECT id FROM n) RETURNING id, body",
    );
    assert_eq!(count, 1);
    assert_eq!(
        result.rows,
        vec![vec![
            CellValue::Integer(3),
            CellValue::Text("IT'S;FINE".into())
        ]]
    );
    drop(session);
    assert!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT id FROM notes WHERE id=3")
            .unwrap()
            .rows
            .is_empty()
    );
}
