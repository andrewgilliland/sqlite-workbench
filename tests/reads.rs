use sqlite_workbench::database::{CellValue, DemoSession, ReadError};

#[test]
fn reads_named_columns_and_sqlite_storage_types() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    let result = session
        .read("SELECT NULL AS missing, 42 AS whole, 1.25 AS fraction, 'hello' AS message, x'00ff' AS bytes")
        .unwrap();
    assert_eq!(
        result.columns,
        ["missing", "whole", "fraction", "message", "bytes"]
    );
    assert_eq!(
        result.rows,
        vec![vec![
            CellValue::Null,
            CellValue::Integer(42),
            CellValue::Real(1.25),
            CellValue::Text("hello".into()),
            CellValue::Blob(vec![0, 255]),
        ]]
    );
    assert!(!result.truncated);
}

#[test]
fn rejects_unsupported_operations_and_scripts_without_changing_notes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let before = session.preview().unwrap().notes;
    for sql in [
        "",
        " \n -- only a comment",
        "/* comment */ ; ;",
        "SELECT 1\0; DELETE FROM notes",
        "INSERT INTO notes(body) VALUES ('bad')",
        "UPDATE notes SET body = 'bad'",
        "DELETE FROM notes",
        "INSERT INTO notes(body) VALUES ('bad') RETURNING id",
        "UPDATE notes SET body = 'bad' RETURNING body",
        "DELETE FROM notes RETURNING id",
        "WITH n AS (SELECT 1) DELETE FROM notes RETURNING id",
        "WITH \"select\" AS (SELECT 1) DELETE FROM notes RETURNING id",
        "WITH édelete AS (SELECT 1) VALUES (2)",
        "WITH explain AS (SELECT 1) SELECT * FROM explain; SELECT 2",
        "WITH édelete AS (SELECT 1) SELECT * FROM édelete; DELETE FROM notes",
        "WITH explain AS (SELECT load_extension('missing')) SELECT * FROM explain",
        "CREATE TABLE unwanted(id)",
        "DROP TABLE notes",
        "ALTER TABLE notes ADD COLUMN extra",
        "PRAGMA user_version = 7",
        "PRAGMA table_info(notes)",
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
        "EXPLAIN SELECT 1",
        "EXPLAIN QUERY PLAN SELECT 1",
        "VALUES (1)",
        "WITH n AS (SELECT 1) VALUES (2)",
        "SELECT 1; SELECT 2",
        "SELECT 1; DELETE FROM notes",
        "INSERT INTO notes(body) VALUES ('bad'); SELECT 1",
        "SELECT load_extension('missing')",
        "SELECT randomblob(10)",
        "SELECT zeroblob(10)",
        "SELECT * FROM pragma_table_info('notes')",
        "SELECT name FROM pragma_table_info('notes')",
        "SELECT * FROM pragma_user_version",
        "SELECT * FROM pragma_database_list",
    ] {
        assert!(
            matches!(session.read(sql), Err(ReadError::Unsupported(_))),
            "{sql:?}"
        );
        assert_eq!(session.preview().unwrap().notes, before, "{sql:?}");
        assert_eq!(
            session.read("SELECT count(*) FROM notes").unwrap().rows,
            vec![vec![CellValue::Integer(2)]],
            "recovery after {sql:?}"
        );
    }
    let external = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        external
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        external
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name = 'unwanted'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn accepts_comments_quoted_semicolons_and_cte_selects() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    for sql in [
        "\u{feff} -- leading comment\n ; /* empty statement */ ; SELECT ';' AS \"semi;colon\"; ;",
        "/* leading comment */ \u{feff} WITH explain AS (SELECT ';' AS v) SELECT v AS \"semi;colon\" FROM explain;",
        "-- SELECT 99;\n SELECT ';' AS \"semi;colon\"; -- trailing ;\n",
        "/* ; DELETE FROM notes */ SELECT ';' AS [semi;colon]; /* ; */",
        "SELECT ';' AS `semi;colon`;",
        "WITH n(v) AS (VALUES (';')) SELECT v AS \"semi;colon\" FROM n;",
        "WITH RECURSIVE n(v) AS (SELECT ';') SELECT v AS \"semi;colon\" FROM n",
    ] {
        let result = session.read(sql).unwrap();
        assert_eq!(result.columns, ["semi;colon"], "{sql}");
        assert_eq!(
            result.rows,
            vec![vec![CellValue::Text(";".into())]],
            "{sql}"
        );
        assert!(!result.truncated);
    }
    assert_eq!(
        session.read("SELECT 'it''s;fine'").unwrap().rows,
        vec![vec![CellValue::Text("it's;fine".into())]]
    );
}

#[test]
fn accepts_explain_as_a_cte_identifier() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    let result = session
        .read("WITH explain AS (SELECT 1 AS n) SELECT n FROM explain;")
        .unwrap();
    assert_eq!(result.columns, ["n"]);
    assert_eq!(result.rows, vec![vec![CellValue::Integer(1)]]);
    assert!(!result.truncated);
}

#[test]
fn accepts_unicode_cte_identifiers_without_matching_keyword_suffixes() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    for name in [
        "édelete",
        "éupdate",
        "éinsert",
        "éexplain",
        "évalues",
        "表2",
        "a$delete",
        "_explain2",
    ] {
        let sql = format!("WITH {name} AS (SELECT 2 AS n) SELECT n FROM {name};");
        let result = session
            .read(&sql)
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
        assert_eq!(result.columns, ["n"], "{sql}");
        assert_eq!(result.rows, vec![vec![CellValue::Integer(2)]], "{sql}");
        assert!(!result.truncated);
    }
}

#[test]
fn accepts_quoted_cte_identifiers_and_escaped_quotes() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    for name in [
        "\"delete\"",
        "[explain]",
        "`values`",
        "\"a\"\"),b\"",
        "`a``),b`",
        "'a''),b'",
    ] {
        let sql = format!("WITH {name}(n) AS (SELECT 3) SELECT n FROM {name};");
        let result = session
            .read(&sql)
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
        assert_eq!(result.columns, ["n"], "{sql}");
        assert_eq!(result.rows, vec![vec![CellValue::Integer(3)]], "{sql}");
        assert!(!result.truncated);
    }
}

#[test]
fn accepts_multiple_ctes_with_optional_materialization_and_nested_selects() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    for sql in [
        "WITH a(n) AS (SELECT 1), explain(n) AS MATERIALIZED (SELECT n + 1 FROM a), édelete(n) AS NOT MATERIALIZED (SELECT n + 1 FROM explain) SELECT n FROM édelete;",
        "WITH RECURSIVE a(n) AS NOT MATERIALIZED (SELECT 1), explain AS (SELECT n + 1 AS n FROM a), b AS MATERIALIZED (SELECT n + 1 AS n FROM explain) SELECT n FROM b;",
        "/* WITH DELETE */ WITH /* name */ explain /* columns */ (n) AS /* hint */ NOT /* hint */ MATERIALIZED /* body */ (SELECT (SELECT 3) /* ), SELECT */ WHERE ')' = ')') /* next */ , b AS (SELECT n FROM explain) /* main */ SELECT n FROM b;",
    ] {
        let result = session
            .read(sql)
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
        assert_eq!(result.columns, ["n"], "{sql}");
        assert_eq!(result.rows, vec![vec![CellValue::Integer(3)]], "{sql}");
        assert!(!result.truncated);
    }
}

#[test]
fn returns_empty_results_and_probes_row_201_for_truncation() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    let empty = session.read("SELECT id, body FROM notes WHERE 0").unwrap();
    assert_eq!(empty.columns, ["id", "body"]);
    assert!(empty.rows.is_empty());
    assert!(!empty.truncated);
    for count in [1, 199, 200, 201, 1000] {
        let result = session.read(&format!(
            "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n WHERE x < {count}) SELECT x FROM n"
        )).unwrap();
        assert_eq!(result.rows.len(), count.min(200));
        assert_eq!(result.truncated, count > 200);
        assert_eq!(result.rows[0], [CellValue::Integer(1)]);
        assert_eq!(
            result.rows.last().unwrap(),
            &[CellValue::Integer(count.min(200) as i64)]
        );
    }
}

#[test]
fn allows_audited_aggregate_string_and_window_functions_and_reports_sql_errors() {
    let directory = tempfile::tempdir().unwrap();
    let mut session = DemoSession::open(directory.path().join("demo.sqlite3")).unwrap();
    assert_eq!(session.read("SELECT count(*), sum(id), upper(substr('hello', 1, 2)), coalesce(NULL, 'fallback'), hex(x'00ff') FROM notes").unwrap().rows,
        vec![vec![CellValue::Integer(2), CellValue::Integer(3), CellValue::Text("HE".into()),
            CellValue::Text("fallback".into()), CellValue::Text("00FF".into())]]);
    assert_eq!(
        session
            .read("SELECT row_number() OVER (ORDER BY id) FROM notes")
            .unwrap()
            .rows,
        vec![vec![CellValue::Integer(1)], vec![CellValue::Integer(2)]]
    );
    for sql in [
        "SELECT FROM notes",
        "SELECT * FROM missing_table",
        "SELECT ?1",
        "SELECT CAST(x'ff' AS TEXT)",
        "SELECT 1; this is invalid",
        "SELECT writefile('bad', 'bad')",
        "WITH 1delete AS (SELECT 1 AS n) SELECT n FROM 1delete",
    ] {
        assert!(matches!(session.read(sql), Err(ReadError::Sql(_))), "{sql}");
        assert_eq!(
            session.read("SELECT 7").unwrap().rows,
            vec![vec![CellValue::Integer(7)]]
        );
    }
}

#[test]
fn exclusive_lock_wait_is_bounded_and_the_same_session_recovers() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external
        .execute_batch("PRAGMA journal_mode = DELETE; BEGIN EXCLUSIVE;")
        .unwrap();
    let started = Instant::now();
    let result = session.read("SELECT id, body FROM notes");
    let elapsed = started.elapsed();
    // Always release the real lock, even if an assertion below fails.
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
    assert_eq!(session.preview().unwrap().notes.len(), 2);
}

#[test]
fn real_recursive_aggregate_times_out_at_production_deadline_and_recovers() {
    use std::time::{Duration, Instant};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute_batch("DELETE FROM notes; WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1200) INSERT INTO notes SELECT x, 'recovery' FROM n;").unwrap();
    let started = Instant::now();
    let result = session.read(
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000000000) SELECT sum(x) FROM n"
    );
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
    // Preview executes enough VM instructions to catch a stale expired hook,
    // before another read installs a fresh one on the same connection.
    let preview = session.preview().unwrap();
    assert_eq!(preview.notes.len(), 200);
    assert!(preview.truncated);
    assert_eq!(
        session.read("SELECT sum(id) FROM notes").unwrap().rows,
        vec![vec![CellValue::Integer(720600)]]
    );
}

#[test]
fn bounds_sql_columns_cells_and_total_copied_payload_without_truncating_cells() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let mut session = DemoSession::open(&path).unwrap();
    let sql = format!("SELECT 1 --{}", "x".repeat(16 * 1024 - "SELECT 1 --".len()));
    assert!(session.read(&sql).is_ok());
    assert!(matches!(
        session.read(&(sql + "x")),
        Err(ReadError::Unsupported(_))
    ));
    for count in [256, 257] {
        let sql = format!("SELECT {}", vec!["1"; count].join(","));
        let result = session.read(&sql);
        if count == 256 {
            assert_eq!(result.unwrap().columns.len(), count);
        } else {
            assert!(matches!(result, Err(ReadError::Unsupported(_))));
        }
    }
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute("DELETE FROM notes", []).unwrap();
    let text = "x".repeat(1024 * 1024);
    for id in 1..=9 {
        external
            .execute(
                "INSERT INTO notes VALUES (?1, ?2)",
                rusqlite::params![id, text],
            )
            .unwrap();
    }
    assert_eq!(
        session
            .read("SELECT body FROM notes WHERE id=1")
            .unwrap()
            .rows,
        vec![vec![CellValue::Text(text.clone())]]
    );
    assert!(session.read("SELECT body FROM notes WHERE id<=7").is_ok());
    assert!(matches!(
        session.read("SELECT body FROM notes"),
        Err(ReadError::Unsupported(_))
    ));
    external
        .execute("UPDATE notes SET body=?1 WHERE id=1", [text + "x"])
        .unwrap();
    assert!(matches!(
        session.read("SELECT body FROM notes WHERE id=1"),
        Err(ReadError::Unsupported(_))
    ));
    external
        .execute(
            "UPDATE notes SET body=?1 WHERE id=1",
            [vec![0u8; 1024 * 1024 + 1]],
        )
        .unwrap();
    assert!(matches!(
        session.read("SELECT body FROM notes WHERE id=1"),
        Err(ReadError::Unsupported(_))
    ));
    assert_eq!(
        session.read("SELECT 42").unwrap().rows,
        vec![vec![CellValue::Integer(42)]]
    );
}
