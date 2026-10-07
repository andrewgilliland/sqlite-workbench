use sqlite_workbench::database::{CellValue, DemoSession, ExecutionResult, ReadError};

#[test]
fn ordinary_writes_report_counts_only_after_commit_and_persist_on_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    for (sql, affected_rows) in [
        ("INSERT INTO notes(body) VALUES ('persistent')", 1),
        (
            "UPDATE notes SET body='verified' WHERE body='persistent'",
            1,
        ),
        ("UPDATE notes SET body='absent' WHERE id=999", 0),
        ("DELETE FROM notes WHERE id=999", 0),
        ("DELETE FROM notes WHERE id=1", 1),
    ] {
        assert_eq!(
            session.execute(sql).unwrap(),
            ExecutionResult::Write {
                affected_rows,
                returned: None
            },
            "{sql}"
        );
    }
    drop(session);
    let mut reopened = DemoSession::open(&path).unwrap();
    let ExecutionResult::Read(result) = reopened
        .execute("SELECT id, body FROM notes ORDER BY id")
        .unwrap()
    else {
        panic!("SELECT must return a read result");
    };
    assert_eq!(result.columns, ["id", "body"]);
    assert_eq!(
        result.rows,
        vec![
            vec![
                CellValue::Integer(2),
                CellValue::Text("Your changes stay in this local file".into())
            ],
            vec![CellValue::Integer(3), CellValue::Text("verified".into())],
        ]
    );
    assert!(!result.truncated);
}

#[test]
fn rejects_scripts_and_out_of_scope_actions_before_any_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external
        .execute_batch("CREATE TABLE other(body TEXT)")
        .unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    for sql in [
        "",
        " -- only comment",
        "; /* only comment */ ;",
        "INSERT INTO notes(body) VALUES ('bad'); SELECT 1",
        "INSERT INTO notes(body) VALUES ('bad'); DELETE FROM notes",
        "SELECT 1; INSERT INTO notes(body) VALUES ('bad')",
        "WITH n AS (SELECT 1) UPDATE notes SET body='bad'; SELECT 2",
        "INSERT INTO notes(body) VALUES ('bad'); BEGIN",
        "CREATE TABLE unwanted(id)",
        "DROP TABLE notes",
        "ALTER TABLE notes ADD COLUMN extra",
        "PRAGMA user_version=7",
        "ATTACH ':memory:' AS extra",
        "DETACH main",
        "BEGIN",
        "COMMIT",
        "ROLLBACK",
        "SAVEPOINT x",
        "RELEASE x",
        "VACUUM",
        "ANALYZE",
        "REINDEX",
        "VALUES (1)",
        "EXPLAIN INSERT INTO notes(body) VALUES ('bad')",
        "REPLACE INTO notes VALUES (1, 'bad')",
        "INSERT INTO other VALUES ('bad')",
        "UPDATE other SET body='bad'",
        "DELETE FROM other",
        "INSERT INTO notes(body) SELECT load_extension('missing')",
        "INSERT INTO notes(body) SELECT randomblob(10)",
        "UPDATE notes SET body=zeroblob(10)",
        "INSERT INTO notes(body) SELECT name FROM pragma_table_info('notes')",
        "INSERT INTO notes(body) VALUES ('bad')\0; DELETE FROM notes",
    ] {
        assert!(
            matches!(session.execute(sql), Err(ReadError::Unsupported(_))),
            "{sql:?}"
        );
        assert_eq!(
            session
                .read("SELECT id, body FROM notes ORDER BY id")
                .unwrap(),
            before,
            "{sql:?}"
        );
        assert_eq!(
            session
                .execute("UPDATE notes SET body=body WHERE 0")
                .unwrap(),
            ExecutionResult::Write {
                affected_rows: 0,
                returned: None
            }
        );
    }
    let oversized = format!(
        "INSERT INTO notes(body) VALUES ('bad') --{}",
        "x".repeat(16 * 1024)
    );
    assert!(matches!(
        session.execute(&oversized),
        Err(ReadError::Unsupported(_))
    ));
    assert_eq!(
        session
            .read("SELECT id, body FROM notes ORDER BY id")
            .unwrap(),
        before
    );
    assert_eq!(
        external
            .query_row("SELECT count(*) FROM other", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        external
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        external
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='unwanted'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn accepts_cte_writes_quoted_comments_and_safe_functions_without_row_limit() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    assert_eq!(
        session.execute("DELETE FROM notes").unwrap(),
        ExecutionResult::Write {
            affected_rows: 2,
            returned: None
        }
    );
    for sql in [
        "\u{feff} -- INSERT ignored;\n ; /* ; */ ; INSERT INTO main.\"notes\"(body) VALUES ('it''s;fine'); ; -- tail ;",
        "WITH \"delete\"(v) AS (VALUES ('cte;one')) INSERT INTO [notes](body) SELECT v FROM \"delete\";",
        "WITH a(v) AS NOT MATERIALIZED (SELECT 'cte;two'), éupdate(v) AS MATERIALIZED (SELECT v FROM a) INSERT INTO `notes`(body) SELECT v FROM éupdate; /* tail ; */",
    ] {
        assert_eq!(
            session.execute(sql).unwrap(),
            ExecutionResult::Write {
                affected_rows: 1,
                returned: None
            },
            "{sql}"
        );
    }
    assert_eq!(session.execute("WITH explain AS (SELECT 'CTE;ONE' AS v) UPDATE notes SET body=upper(body) WHERE upper(body)=(SELECT v FROM explain)").unwrap(), ExecutionResult::Write { affected_rows: 1, returned: None });
    assert_eq!(session.execute("WITH édelete AS (SELECT 'cte;two' AS v) DELETE FROM notes WHERE body=(SELECT v FROM édelete)").unwrap(), ExecutionResult::Write { affected_rows: 1, returned: None });
    assert_eq!(
        session
            .read("SELECT body FROM notes ORDER BY id")
            .unwrap()
            .rows,
        vec![
            vec![CellValue::Text("it's;fine".into())],
            vec![CellValue::Text("CTE;ONE".into())]
        ]
    );
    assert_eq!(session.execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000) INSERT INTO notes(body) SELECT 'bulk' FROM n").unwrap(), ExecutionResult::Write { affected_rows: 1000, returned: None });
    assert_eq!(
        session
            .read("SELECT count(*) FROM notes WHERE body='bulk'")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(1000)]]
    );
    let ExecutionResult::Read(result) = session
        .execute("SELECT body FROM notes WHERE body='bulk'")
        .unwrap()
    else {
        panic!("expected SELECT result");
    };
    assert_eq!(result.rows.len(), 200);
    assert!(result.truncated);
}

#[test]
fn deleting_all_notes_and_reopening_never_reseeds() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    assert_eq!(
        session.execute("DELETE FROM notes").unwrap(),
        ExecutionResult::Write {
            affected_rows: 2,
            returned: None
        }
    );
    drop(session);
    let mut reopened = DemoSession::open(&path).unwrap();
    assert!(
        reopened
            .read("SELECT id, body FROM notes")
            .unwrap()
            .rows
            .is_empty()
    );
    assert_eq!(
        reopened.execute("DELETE FROM notes").unwrap(),
        ExecutionResult::Write {
            affected_rows: 0,
            returned: None
        }
    );
    assert_eq!(
        reopened
            .execute("INSERT INTO notes(body) VALUES ('fresh')")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 1,
            returned: None
        }
    );
    drop(reopened);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT id, body FROM notes")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(1), CellValue::Text("fresh".into())]]
    );
}

#[test]
fn constraint_failures_rollback_partial_changes_and_ignore_commits_selected_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute_batch("DELETE FROM notes; INSERT INTO notes VALUES (1, 'first'), (2, 'second'); CREATE UNIQUE INDEX unique_body ON notes(body);").unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    for sql in [
        "UPDATE OR FAIL notes SET body='collision'",
        "UPDATE OR ROLLBACK notes SET body='collision'",
        "INSERT OR FAIL INTO notes VALUES (3, 'third'), (4, 'first')",
        "INSERT OR ROLLBACK INTO notes VALUES (3, 'third'), (4, 'first')",
        "INSERT INTO notes VALUES (3, 'third'), (4, 'first')",
    ] {
        let result = session.execute(sql);
        assert!(
            matches!(result, Err(ReadError::Sql(ref error)) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation)),
            "{sql}: {result:?}"
        );
        assert_eq!(
            session
                .read("SELECT id, body FROM notes ORDER BY id")
                .unwrap(),
            before,
            "{sql}"
        );
        assert_eq!(
            session
                .execute("UPDATE notes SET body=body WHERE 0")
                .unwrap(),
            ExecutionResult::Write {
                affected_rows: 0,
                returned: None
            }
        );
        assert_eq!(
            DemoSession::open(&path)
                .unwrap()
                .read("SELECT id, body FROM notes ORDER BY id")
                .unwrap(),
            before
        );
    }
    assert_eq!(
        session
            .execute("INSERT OR IGNORE INTO notes VALUES (3, 'third'), (4, 'first'), (5, 'fifth')")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 2,
            returned: None
        }
    );
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT body FROM notes ORDER BY id")
            .unwrap()
            .rows,
        vec![
            vec![CellValue::Text("first".into())],
            vec![CellValue::Text("second".into())],
            vec![CellValue::Text("third".into())],
            vec![CellValue::Text("fifth".into())]
        ]
    );
}

#[test]
fn invalid_sql_and_invalid_tails_are_sql_errors_without_first_statement_effects() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    for sql in [
        "INSERT INTO notes(body) VALUES",
        "INSERT INTO missing_table VALUES ('bad')",
        "UPDATE notes SET missing_column=1",
        "INSERT INTO notes(body) VALUES (?1)",
        "INSERT INTO notes(body) VALUES ('bad'); this is invalid",
        "INSERT INTO notes(body) VALUES ('bad'); SELECT FROM notes",
        "WITH 1delete AS (SELECT 1) INSERT INTO notes(body) VALUES ('bad')",
    ] {
        assert!(
            matches!(session.execute(sql), Err(ReadError::Sql(_))),
            "{sql}"
        );
        assert_eq!(
            session
                .read("SELECT id, body FROM notes ORDER BY id")
                .unwrap(),
            before,
            "{sql}"
        );
        assert_eq!(
            session
                .execute("UPDATE notes SET body=body WHERE 0")
                .unwrap(),
            ExecutionResult::Write {
                affected_rows: 0,
                returned: None
            }
        );
    }
}

#[test]
fn commit_lock_failure_rolls_back_before_returning_and_the_session_recovers() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    // An actual read inside a deferred transaction holds SHARED. INSERT may
    // acquire RESERVED and finish stepping, but COMMIT cannot get EXCLUSIVE.
    external
        .execute_batch("PRAGMA journal_mode=DELETE; BEGIN DEFERRED; SELECT * FROM notes;")
        .unwrap();
    let started = Instant::now();
    let result = session.execute("INSERT INTO notes(body) VALUES ('must rollback')");
    let elapsed = started.elapsed();
    let external_count = external
        .query_row("SELECT count(*) FROM notes", [], |row| row.get::<_, i64>(0))
        .unwrap();
    external.execute_batch("ROLLBACK").unwrap();
    assert!(matches!(result, Err(ReadError::Locked)), "{result:?}");
    assert!(
        elapsed >= Duration::from_millis(900),
        "lock wait too short: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "lock wait unbounded: {elapsed:?}"
    );
    assert_eq!(external_count, 2);
    // This independent write also proves no leaked RESERVED transaction remains.
    external
        .execute("INSERT INTO notes(body) VALUES ('external')", [])
        .unwrap();
    assert_eq!(
        session
            .read("SELECT body FROM notes ORDER BY id")
            .unwrap()
            .rows,
        vec![
            vec![CellValue::Text("Welcome to SQLite Workbench".into())],
            vec![CellValue::Text(
                "Your changes stay in this local file".into()
            )],
            vec![CellValue::Text("external".into())]
        ]
    );
    assert_eq!(
        session
            .execute("INSERT INTO notes(body) VALUES ('recovered')")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 1,
            returned: None
        }
    );
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
fn exclusive_lock_during_write_preparation_is_bounded_and_recovers() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external
        .execute_batch("PRAGMA journal_mode=DELETE; BEGIN EXCLUSIVE;")
        .unwrap();
    let started = Instant::now();
    let result = session.execute("INSERT INTO notes(body) VALUES ('locked')");
    let elapsed = started.elapsed();
    external.execute_batch("ROLLBACK").unwrap();
    assert!(matches!(result, Err(ReadError::Locked)), "{result:?}");
    assert!(
        elapsed >= Duration::from_millis(900),
        "lock wait too short: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "lock wait unbounded: {elapsed:?}"
    );
    assert_eq!(
        session.read("SELECT count(*) FROM notes").unwrap().rows,
        vec![vec![CellValue::Integer(2)]]
    );
    assert_eq!(
        session
            .execute("INSERT INTO notes(body) VALUES ('recovered')")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 1,
            returned: None
        }
    );
    assert_eq!(
        session.read("SELECT count(*) FROM notes").unwrap().rows,
        vec![vec![CellValue::Integer(3)]]
    );
}

#[test]
fn real_recursive_insert_times_out_rolls_back_and_cleans_hooks_before_returning() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    let started = Instant::now();
    let result = session.execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000000000) INSERT INTO notes(body) SELECT 'deadline' FROM n");
    let elapsed = started.elapsed();
    assert!(matches!(result, Err(ReadError::Timeout)), "{result:?}");
    assert!(
        elapsed >= Duration::from_secs(5),
        "did not use production deadline: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(8),
        "deadline overshoot: {elapsed:?}"
    );
    let external = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        external
            .query_row("SELECT count(*) FROM notes", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        2
    );
    // This independent fixture write checks that rollback released write locks.
    external.execute_batch("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1200) INSERT INTO notes(body) SELECT 'external' FROM n").unwrap();
    // Enough VM work to catch stale hooks, before execute installs new ones.
    let preview = session.preview().unwrap();
    assert_eq!(preview.notes.len(), 200);
    assert!(preview.truncated);
    assert_eq!(
        session
            .read("SELECT id, body FROM notes WHERE id<=2 ORDER BY id")
            .unwrap(),
        before
    );
    assert_eq!(
        session
            .execute("INSERT INTO notes(body) VALUES ('recovered')")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 1,
            returned: None
        }
    );
    assert_eq!(
        session
            .read("SELECT count(*) FROM notes WHERE body='deadline'")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(0)]]
    );
    drop(session);
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .read("SELECT count(*) FROM notes")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(1203)]]
    );
}

#[test]
fn triggers_may_write_notes_but_cannot_write_other_tables() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute_batch("CREATE TRIGGER normalize_note AFTER INSERT ON notes BEGIN UPDATE notes SET body=upper(new.body) WHERE id=new.id; END;").unwrap();
    assert_eq!(
        session
            .execute("INSERT INTO notes(body) VALUES ('triggered')")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 1,
            returned: None
        }
    );
    assert_eq!(
        session
            .read("SELECT body FROM notes WHERE id=3")
            .unwrap()
            .rows,
        vec![vec![CellValue::Text("TRIGGERED".into())]]
    );
    external.execute_batch("CREATE TABLE other(body TEXT); CREATE TRIGGER unsafe_note AFTER INSERT ON notes BEGIN INSERT INTO other(body) VALUES (new.body); END;").unwrap();
    let before = session
        .read("SELECT id, body FROM notes ORDER BY id")
        .unwrap();
    assert!(matches!(
        session.execute("INSERT INTO notes(body) VALUES ('unsafe')"),
        Err(ReadError::Unsupported(_))
    ));
    assert_eq!(
        session
            .read("SELECT id, body FROM notes ORDER BY id")
            .unwrap(),
        before
    );
    assert_eq!(
        external
            .query_row("SELECT count(*) FROM other", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        session
            .execute("UPDATE notes SET body='recovered' WHERE id=3")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 1,
            returned: None
        }
    );
    assert_eq!(
        session
            .read("SELECT body FROM notes WHERE id=3")
            .unwrap()
            .rows,
        vec![vec![CellValue::Text("recovered".into())]]
    );
}

#[test]
fn reserved_write_lock_rolls_back_the_started_transaction_and_recovers() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    // RESERVED permits preparation and our internal deferred BEGIN, but the
    // user write cannot acquire its own RESERVED lock while stepping.
    external
        .execute_batch("PRAGMA journal_mode=DELETE; BEGIN IMMEDIATE;")
        .unwrap();
    let started = Instant::now();
    let result = session.execute("UPDATE notes SET body='locked'");
    let elapsed = started.elapsed();
    external.execute_batch("ROLLBACK").unwrap();
    assert!(matches!(result, Err(ReadError::Locked)), "{result:?}");
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
            .read("SELECT body FROM notes WHERE id=1")
            .unwrap()
            .rows,
        vec![vec![CellValue::Text("Welcome to SQLite Workbench".into())]]
    );
    assert_eq!(
        session
            .execute("UPDATE notes SET body='recovered' WHERE id=1")
            .unwrap(),
        ExecutionResult::Write {
            affected_rows: 1,
            returned: None
        }
    );
    assert_eq!(
        session
            .read("SELECT body FROM notes WHERE id=1")
            .unwrap()
            .rows,
        vec![vec![CellValue::Text("recovered".into())]]
    );
}
