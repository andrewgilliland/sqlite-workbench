use gpui_kit::component::ActiveTheme;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, ElementId, Point, TestAppContext, Window, WindowBounds,
    WindowOptions, px, size,
};
use sqlite_workbench::shell::Workbench;
use std::{path::PathBuf, time::Duration};

fn open_window(cx: &mut TestAppContext, path: PathBuf) -> AnyWindowHandle {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1100.0), px(720.0)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| Workbench::with_database_path(path)),
        )
        .unwrap()
        .0
    })
}

fn expect_label(window: &Window, id: impl Into<ElementId>, expected: &str) {
    assert_eq!(window.find(id).label(), Some(expected));
}

async fn wait_status(cx: &mut TestAppContext, handle: AnyWindowHandle, status: &str) {
    cx.wait_for(handle, Duration::from_secs(15), |window, _| {
        window.find("connection-status").label() == Some(status)
    })
    .await;
}

async fn connect(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-database", cx);
        window.click("open-database", cx);
        window.click("run-query", cx);
        expect_label(window, "connection-status", "Opening demo database…");
    })
    .unwrap();
    wait_status(cx, handle, "Demo database connected").await;
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, sql: &str) {
    cx.update_window(handle, |_, window, cx| {
        window.click("sql-editor", cx);
        window.press("secondary-a", cx);
        window.input(sql, cx);
        window.click("run-query", cx);
        expect_label(window, "connection-status", "Running query…");
        expect_label(window, "result-summary", "No query results");
        assert!(window.try_find(("result-column", 0_u64)).is_none());
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn insert_returning_shows_committed_columns_and_cells(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let handle = open_window(cx, directory.path().join("demo.sqlite3"));
    connect(cx, handle).await;
    run(
        cx,
        handle,
        "INSERT INTO notes(body) VALUES ('returned') RETURNING id, body;",
    );
    wait_status(cx, handle, "Write committed: 1 rows affected").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, ("result-column", 0_u64), "id");
        expect_label(window, ("result-column", 1_u64), "body");
        expect_label(window, ("result-cell", 0_u64), "3");
        expect_label(window, ("result-cell", 1_u64), "\"returned\"");
        expect_label(
            window,
            "result-summary",
            "Write committed: 1 rows affected; 1 rows returned",
        );
    })
    .unwrap();
}

#[gpui_kit::test]
async fn returning_limits_display_not_committed_crud_at_199_200_201_and_1201_rows(
    cx: &mut TestAppContext,
) {
    for count in [199, 200, 201, 1201] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("demo.sqlite3");
        let handle = open_window(cx, path.clone());
        connect(cx, handle).await;
        returning_rows(
            cx,
            handle,
            &format!("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<{count}) INSERT INTO notes(id, body) SELECT x+100, 'inserted' FROM n RETURNING body;"),
            count,
            "\"inserted\"",
        )
        .await;
        let external = rusqlite::Connection::open(&path).unwrap();
        expect_count(
            &external,
            "SELECT count(*) FROM notes WHERE body='inserted'",
            count,
        );
        returning_rows(
            cx,
            handle,
            "UPDATE notes SET body='updated' WHERE id>100 RETURNING body;",
            count,
            "\"updated\"",
        )
        .await;
        expect_count(
            &external,
            "SELECT count(*) FROM notes WHERE body='updated'",
            count,
        );
        expect_count(
            &external,
            "SELECT count(*) FROM notes WHERE body='inserted'",
            0,
        );
        close_window(cx, handle);
        let reopened = open_window(cx, path.clone());
        connect(cx, reopened).await;
        read_count(
            cx,
            reopened,
            "SELECT count(*) FROM notes WHERE body='updated';",
            count,
        )
        .await;
        returning_rows(
            cx,
            reopened,
            "DELETE FROM notes WHERE id>100 RETURNING body;",
            count,
            "\"updated\"",
        )
        .await;
        expect_count(&external, "SELECT count(*) FROM notes", 2);
        close_window(cx, reopened);
        let after_delete = open_window(cx, path);
        connect(cx, after_delete).await;
        read_count(cx, after_delete, "SELECT count(*) FROM notes;", 2).await;
        close_window(cx, after_delete);
    }
}

async fn returning_rows(
    cx: &mut TestAppContext,
    handle: AnyWindowHandle,
    sql: &str,
    count: usize,
    value: &str,
) {
    run(cx, handle, sql);
    let status = format!(
        "Write committed: {count} rows affected{}",
        if count > 200 {
            " (returned rows truncated to 200)"
        } else {
            ""
        },
    );
    cx.wait_for(handle, Duration::from_secs(15), |window, _| {
        window
            .find("connection-status")
            .label()
            .is_some_and(|label| label.starts_with("Write committed:"))
    })
    .await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, "connection-status", &status);
        let displayed = count.min(200);
        let summary = format!(
            "Write committed: {count} rows affected; {displayed} rows returned{}",
            if count > 200 {
                " (truncated to 200 rows)"
            } else {
                ""
            },
        );
        expect_label(window, "result-summary", &summary);
        expect_label(window, ("result-column", 0_u64), "body");
        expect_label(window, ("result-cell", 0_u64), value);
        expect_label(
            window,
            ("result-cell", ((displayed - 1) * 256) as u64),
            value,
        );
        assert!(window.try_find(("result-row", displayed as u64)).is_none());
        // All returned bodies are identical: no assumption about SQLite's row order.
        let rows = gpui_kit::base::test_support::snapshots(window)
            .into_iter()
            .filter(|element| element.role() == Some(gpui_kit::Role::Cell))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), displayed);
        assert!(rows.iter().all(|cell| cell.label() == Some(value)));
    })
    .unwrap();
}

fn expect_count(connection: &rusqlite::Connection, sql: &str, expected: usize) {
    let count: i64 = connection.query_row(sql, [], |row| row.get(0)).unwrap();
    assert_eq!(count, expected as i64);
}

async fn read_count(cx: &mut TestAppContext, handle: AnyWindowHandle, sql: &str, count: usize) {
    run(cx, handle, sql);
    wait_status(cx, handle, "Query complete: 1 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, "result-summary", "1 rows");
        expect_label(window, ("result-cell", 0_u64), &count.to_string());
    })
    .unwrap();
}

fn close_window(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.remove_window();
        cx.refresh_windows();
    })
    .unwrap();
}

#[gpui_kit::test]
async fn typed_returning_crud_and_empty_headers_preserve_theme_editor_and_session(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    let insert = "INSERT INTO notes(id, body) VALUES (100, NULL) RETURNING id, body, 'NULL' AS literal, 1.5 AS real_value, X'00ABFF' AS bytes, 'quote\"' AS quoted;";
    run(cx, handle, insert);
    wait_status(cx, handle, "Write committed: 1 rows affected").await;
    cx.update_window(handle, |_, window, cx| {
        for (index, name) in ["id", "body", "literal", "real_value", "bytes", "quoted"]
            .iter()
            .enumerate()
        {
            expect_label(window, ("result-column", index as u64), name);
        }
        for (index, value) in [
            "100",
            "SQL NULL",
            "\"NULL\"",
            "1.5",
            "X'00ABFF'",
            "\"quote\\\"\"",
        ]
        .iter()
        .enumerate()
        {
            expect_label(window, ("result-cell", index as u64), value);
            assert_eq!(
                window.find(("result-cell", index as u64)).role(),
                Some(gpui_kit::Role::Cell)
            );
        }
        let was_dark = cx.theme().is_dark();
        window.click("theme-toggle", cx);
        assert_ne!(cx.theme().is_dark(), was_dark);
        assert_eq!(editor_value(window), insert);
        expect_label(window, "database-path", path.to_str().unwrap());
        expect_label(
            window,
            "connection-status",
            "Write committed: 1 rows affected",
        );
        expect_label(
            window,
            "result-summary",
            "Write committed: 1 rows affected; 1 rows returned",
        );
        expect_label(window, ("result-cell", 1_u64), "SQL NULL");
        window.click("open-database", cx);
        expect_label(window, ("result-cell", 4_u64), "X'00ABFF'");
    })
    .unwrap();
    returning_rows(
        cx,
        handle,
        "UPDATE notes SET body='changed' WHERE id=100 RETURNING body;",
        1,
        "\"changed\"",
    )
    .await;
    returning_rows(
        cx,
        handle,
        "DELETE FROM notes WHERE id=100 RETURNING body;",
        1,
        "\"changed\"",
    )
    .await;
    let external = rusqlite::Connection::open(&path).unwrap();
    expect_count(&external, "SELECT count(*) FROM notes WHERE id=100", 0);

    for sql in [
        "UPDATE notes SET body='absent' WHERE id=999 RETURNING id AS empty_id, body AS empty_body;",
        "DELETE FROM notes WHERE id=999 RETURNING id AS empty_id, body AS empty_body;",
        "INSERT INTO notes(body) SELECT 'absent' WHERE 0 RETURNING id AS empty_id, body AS empty_body;",
    ] {
        run(cx, handle, sql);
        wait_status(cx, handle, "Write committed: 0 rows affected").await;
        cx.update_window(handle, |_, window, cx| {
            expect_label(
                window,
                "result-summary",
                "Write committed: 0 rows affected; No rows returned",
            );
            expect_label(window, ("result-column", 0_u64), "empty_id");
            expect_label(window, ("result-column", 1_u64), "empty_body");
            assert!(window.try_find(("result-column", 2_u64)).is_none());
            assert!(window.try_find(("result-row", 0_u64)).is_none());
            assert!(window.try_find(("result-cell", 0_u64)).is_none());
            assert!(window.try_find(("result-cell", 1_u64)).is_none());
            let was_dark = cx.theme().is_dark();
            window.click("theme-toggle", cx);
            assert_ne!(cx.theme().is_dark(), was_dark);
            assert_eq!(editor_value(window), sql);
            expect_label(window, ("result-column", 1_u64), "empty_body");
            expect_label(
                window,
                "connection-status",
                "Write committed: 0 rows affected",
            );
            expect_label(window, "database-path", path.to_str().unwrap());
        })
        .unwrap();
        expect_count(&external, "SELECT count(*) FROM notes", 2);
    }
    close_window(cx, handle);
    let reopened = open_window(cx, path);
    connect(cx, reopened).await;
    read_count(cx, reopened, "SELECT count(*) FROM notes;", 2).await;
}

fn editor_value(window: &Window) -> String {
    gpui_kit::base::test_support::snapshots(window)
        .into_iter()
        .find(|element| element.label() == Some("SQL editor"))
        .unwrap()
        .value()
        .unwrap()
        .to_owned()
}

fn expect_no_results(window: &Window) {
    expect_label(window, "result-summary", "No query results");
    assert!(window.try_find(("result-column", 0_u64)).is_none());
    assert!(window.try_find(("result-row", 0_u64)).is_none());
    assert!(window.try_find(("result-cell", 0_u64)).is_none());
}

#[gpui_kit::test]
async fn returning_errors_discard_collected_rows_and_partial_writes_before_retry(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    returning_rows(
        cx,
        handle,
        "WITH RECURSIVE n(x) AS (SELECT 3 UNION ALL SELECT x+1 FROM n WHERE x<9) INSERT INTO notes(id, body) SELECT x, 'seed' FROM n RETURNING body;",
        7,
        "\"seed\"",
    )
    .await;
    let external = rusqlite::Connection::open(&path).unwrap();
    for (sql, category) in [
        // Each cell fits, but the ninth exceeds the aggregate copy budget after
        // rows have been collected. No partial returned table may reach the UI.
        (
            "UPDATE notes SET body='must rollback' RETURNING id, replace(replace(replace(replace(replace(replace('x','x','xxxxxxxxxx'),'x','xxxxxxxxxx'),'x','xxxxxxxxxx'),'x','xxxxxxxxxx'),'x','xxxxxxxxxx'),'x','xxxxxxxxxx') AS payload;",
            "Unsupported SQL:",
        ),
        (
            "INSERT OR FAIL INTO notes(id, body) VALUES (100, 'must rollback'), (1, 'collision') RETURNING id, body;",
            "Query error:",
        ),
        (
            "INSERT INTO notes(id, body) VALUES (100, 'must rollback') RETURNING id; DELETE FROM notes;",
            "Unsupported SQL:",
        ),
        (
            "INSERT INTO notes(id, body) VALUES (100, 'must rollback') RETURNING id; SELECT FROM notes;",
            "Query error:",
        ),
    ] {
        returning_rows(
            cx,
            handle,
            "UPDATE notes SET body=body WHERE id=3 RETURNING body;",
            1,
            "\"seed\"",
        )
        .await;
        run(cx, handle, sql);
        cx.wait_for(handle, Duration::from_secs(15), |window, _| {
            window
                .find("connection-status")
                .label()
                .is_some_and(|label| label.starts_with(category))
        })
        .await;
        cx.update_window(handle, |_, window, cx| {
            if sql.contains(" AS payload;") {
                assert!(
                    window
                        .find("connection-status")
                        .label()
                        .unwrap()
                        .contains("result exceeds 8 MiB")
                );
            }
            expect_no_results(window);
            expect_label(window, "database-path", path.to_str().unwrap());
            let status = window.find("connection-status").label().unwrap().to_owned();
            window.click("open-database", cx);
            expect_label(window, "connection-status", &status);
        })
        .unwrap();
        expect_count(&external, "SELECT count(*) FROM notes", 9);
        expect_count(
            &external,
            "SELECT count(*) FROM notes WHERE body='must rollback' OR id=100",
            0,
        );
        expect_count(&external, "SELECT count(*) FROM notes WHERE body='seed'", 7);
        read_count(cx, handle, "SELECT count(*) FROM notes;", 9).await;
        returning_rows(
            cx,
            handle,
            "INSERT INTO notes(id, body) VALUES (100, 'retry') RETURNING body;",
            1,
            "\"retry\"",
        )
        .await;
        expect_count(
            &external,
            "SELECT count(*) FROM notes WHERE id=100 AND body='retry'",
            1,
        );
        returning_rows(
            cx,
            handle,
            "DELETE FROM notes WHERE id=100 RETURNING body;",
            1,
            "\"retry\"",
        )
        .await;
    }
    close_window(cx, handle);
    let reopened = open_window(cx, path);
    connect(cx, reopened).await;
    read_count(cx, reopened, "SELECT count(*) FROM notes;", 9).await;
}

#[gpui_kit::test]
async fn commit_lock_never_presents_collected_returning_rows_and_retry_commits_once(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    returning_rows(
        cx,
        handle,
        "UPDATE notes SET body=body WHERE id=1 RETURNING body;",
        1,
        "\"Welcome to SQLite Workbench\"",
    )
    .await;
    let external = rusqlite::Connection::open(&path).unwrap();
    // SHARED allows RETURNING to be collected but prevents COMMIT acquiring
    // EXCLUSIVE. The final public execution outcome must remain an error.
    external
        .execute_batch("PRAGMA journal_mode=DELETE; BEGIN; SELECT id FROM notes;")
        .unwrap();
    let insert = "INSERT INTO notes(id, body) VALUES (100, 'commit retry') RETURNING id, body;";
    run(cx, handle, insert);
    let dark = cx
        .update_window(handle, |_, window, cx| {
            for _ in 0..3 {
                window.click("run-query", cx);
                window.click("open-database", cx);
            }
            let was_dark = cx.theme().is_dark();
            window.click("theme-toggle", cx);
            assert_ne!(cx.theme().is_dark(), was_dark);
            assert_eq!(editor_value(window), insert);
            expect_label(window, "connection-status", "Running query…");
            expect_no_results(window);
            cx.theme().is_dark()
        })
        .unwrap();
    wait_status(cx, handle, "Database locked").await;
    cx.update_window(handle, |_, window, cx| {
        expect_no_results(window);
        assert_eq!(cx.theme().is_dark(), dark);
        assert_eq!(editor_value(window), insert);
        expect_label(window, "database-path", path.to_str().unwrap());
        window.click("open-database", cx);
        expect_label(window, "connection-status", "Database locked");
    })
    .unwrap();
    expect_count(&external, "SELECT count(*) FROM notes WHERE id=100", 0);
    external.execute_batch("ROLLBACK;").unwrap();
    read_count(cx, handle, "SELECT count(*) FROM notes;", 2).await;
    run(cx, handle, insert);
    wait_status(cx, handle, "Write committed: 1 rows affected").await;
    cx.update_window(handle, |_, window, cx| {
        expect_label(
            window,
            "result-summary",
            "Write committed: 1 rows affected; 1 rows returned",
        );
        expect_label(window, ("result-column", 0_u64), "id");
        expect_label(window, ("result-column", 1_u64), "body");
        expect_label(window, ("result-cell", 0_u64), "100");
        expect_label(window, ("result-cell", 1_u64), "\"commit retry\"");
        assert_eq!(cx.theme().is_dark(), dark);
    })
    .unwrap();
    expect_count(
        &external,
        "SELECT count(*) FROM notes WHERE id=100 AND body='commit retry'",
        1,
    );
    expect_count(&external, "SELECT count(*) FROM notes", 3);
    close_window(cx, handle);
    let reopened = open_window(cx, path);
    connect(cx, reopened).await;
    read_count(cx, reopened, "SELECT count(*) FROM notes WHERE id=100;", 1).await;
}

#[gpui_kit::test]
async fn returning_timeout_rolls_back_and_busy_edits_never_queue_a_write_or_reset_theme(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    returning_rows(
        cx,
        handle,
        "UPDATE notes SET body=body WHERE id=1 RETURNING body;",
        1,
        "\"Welcome to SQLite Workbench\"",
    )
    .await;
    let started = std::time::Instant::now();
    run(
        cx,
        handle,
        "WITH RECURSIVE slow(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM slow WHERE n<1000000000) INSERT INTO notes(body) SELECT 'deadline rollback' FROM slow RETURNING id, body;",
    );
    let recovery = "INSERT INTO notes(body) VALUES ('captured retry') RETURNING id, body;";
    let dark = cx
        .update_window(handle, |_, window, cx| {
            for _ in 0..3 {
                window.click("run-query", cx);
                window.click("open-database", cx);
            }
            window.click("sql-editor", cx);
            window.press("secondary-a", cx);
            window.input(recovery, cx);
            for _ in 0..3 {
                window.click("run-query", cx);
                window.click("open-database", cx);
            }
            let was_dark = cx.theme().is_dark();
            window.click("theme-toggle", cx);
            assert_ne!(cx.theme().is_dark(), was_dark);
            assert_eq!(editor_value(window), recovery);
            expect_label(window, "connection-status", "Running query…");
            expect_no_results(window);
            expect_label(window, "database-path", path.to_str().unwrap());
            cx.theme().is_dark()
        })
        .unwrap();
    wait_status(cx, handle, "Query timed out").await;
    assert!(started.elapsed() >= Duration::from_secs(5));
    cx.update_window(handle, |_, window, cx| {
        expect_no_results(window);
        assert_eq!(editor_value(window), recovery);
        assert_eq!(cx.theme().is_dark(), dark);
        window.click("open-database", cx);
        expect_label(window, "connection-status", "Query timed out");
    })
    .unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    expect_count(&external, "SELECT count(*) FROM notes", 2);
    expect_count(
        &external,
        "SELECT count(*) FROM notes WHERE body IN ('deadline rollback', 'captured retry')",
        0,
    );
    // Retry the retained native editor value, not a replacement statement.
    cx.update_window(handle, |_, window, cx| {
        window.click("run-query", cx);
        window.click("run-query", cx);
        expect_label(window, "connection-status", "Running query…");
        expect_no_results(window);
    })
    .unwrap();
    wait_status(cx, handle, "Write committed: 1 rows affected").await;
    cx.update_window(handle, |_, window, cx| {
        expect_label(
            window,
            "result-summary",
            "Write committed: 1 rows affected; 1 rows returned",
        );
        expect_label(window, ("result-column", 0_u64), "id");
        expect_label(window, ("result-column", 1_u64), "body");
        expect_label(window, ("result-cell", 0_u64), "3");
        expect_label(window, ("result-cell", 1_u64), "\"captured retry\"");
        assert_eq!(editor_value(window), recovery);
        assert_eq!(cx.theme().is_dark(), dark);
    })
    .unwrap();
    expect_count(&external, "SELECT count(*) FROM notes", 3);
    expect_count(
        &external,
        "SELECT count(*) FROM notes WHERE body='captured retry'",
        1,
    );
    expect_count(
        &external,
        "SELECT count(*) FROM notes WHERE body='deadline rollback'",
        0,
    );
    read_count(cx, handle, "SELECT count(*) FROM notes;", 3).await;
    close_window(cx, handle);
    let reopened = open_window(cx, path);
    connect(cx, reopened).await;
    read_count(
        cx,
        reopened,
        "SELECT count(*) FROM notes WHERE body='captured retry';",
        1,
    )
    .await;
    read_count(
        cx,
        reopened,
        "SELECT count(*) FROM notes WHERE body='deadline rollback';",
        0,
    )
    .await;
}
