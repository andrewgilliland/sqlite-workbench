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

#[gpui_kit::test]
async fn shows_empty_results_and_distinguishes_null_text_and_scalar_values(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let handle = open_window(cx, directory.path().join("demo.sqlite3"));
    connect(cx, handle).await;
    run(
        cx,
        handle,
        "SELECT NULL AS missing, 'NULL' AS literal, 42 AS integer_value, 1.5 AS real_value, X'00ABFF' AS bytes, '(SQL NULL)' AS marker, 'SQL NULL' AS accessible_marker;",
    );
    wait_status(cx, handle, "Query complete: 1 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, ("result-column", 0_u64), "missing");
        expect_label(window, ("result-column", 1_u64), "literal");
        expect_label(window, ("result-cell", 0_u64), "SQL NULL");
        expect_label(window, ("result-cell", 1_u64), "\"NULL\"");
        expect_label(window, ("result-cell", 2_u64), "42");
        expect_label(window, ("result-cell", 3_u64), "1.5");
        expect_label(window, ("result-cell", 4_u64), "X'00ABFF'");
        expect_label(window, ("result-cell", 5_u64), "\"(SQL NULL)\"");
        expect_label(window, ("result-cell", 6_u64), "\"SQL NULL\"");
        assert_eq!(
            window.find(("result-row", 0_u64)).role(),
            Some(gpui_kit::Role::Row)
        );
        assert_eq!(
            window.find(("result-cell", 0_u64)).role(),
            Some(gpui_kit::Role::Cell)
        );
    })
    .unwrap();
    run(
        cx,
        handle,
        "SELECT body AS empty_column FROM notes WHERE id = -1;",
    );
    wait_status(cx, handle, "Query complete: 0 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, "result-summary", "No rows returned");
        expect_label(window, ("result-column", 0_u64), "empty_column");
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn clears_stale_results_and_recovers_from_unsupported_and_invalid_sql(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    for (sql, category) in [
        (
            "UPDATE notes SET body = 'must not change';",
            "Unsupported SQL:",
        ),
        (
            "DELETE FROM notes; SELECT id FROM notes;",
            "Unsupported SQL:",
        ),
        ("SELECT FROM notes;", "Query error:"),
    ] {
        run(cx, handle, "SELECT body FROM notes ORDER BY id;");
        wait_status(cx, handle, "Query complete: 2 rows").await;
        run(cx, handle, sql);
        cx.wait_for(handle, Duration::from_secs(10), |window, _| {
            window
                .find("connection-status")
                .label()
                .is_some_and(|label| label.starts_with(category))
        })
        .await;
        cx.update_window(handle, |_, window, cx| {
            assert!(window.try_find(("result-column", 0_u64)).is_none());
            assert!(window.try_find(("result-cell", 0_u64)).is_none());
            expect_label(window, "result-summary", "No query results");
            expect_label(window, "database-path", path.to_str().unwrap());
            let status = window.find("connection-status").label().unwrap().to_owned();
            window.click("open-database", cx);
            expect_label(window, "connection-status", &status);
        })
        .unwrap();
    }
    run(cx, handle, "SELECT body FROM notes ORDER BY id;");
    wait_status(cx, handle, "Query complete: 2 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(
            window,
            ("result-cell", 0_u64),
            "\"Welcome to SQLite Workbench\"",
        );
        expect_label(
            window,
            ("result-cell", 256_u64),
            "\"Your changes stay in this local file\"",
        );
    })
    .unwrap();
}

#[gpui_kit::test]
async fn distinguishes_exactly_200_rows_from_truncation(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let handle = open_window(cx, directory.path().join("demo.sqlite3"));
    connect(cx, handle).await;
    for (sql, status, summary) in [
        (
            "WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM numbers WHERE n < 201) SELECT n FROM numbers;",
            "Query complete: 200 rows (truncated to 200 rows)",
            "200 rows (truncated to 200 rows)",
        ),
        (
            "WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM numbers WHERE n < 200) SELECT n FROM numbers;",
            "Query complete: 200 rows",
            "200 rows",
        ),
    ] {
        run(cx, handle, sql);
        wait_status(cx, handle, status).await;
        cx.update_window(handle, |_, window, _| {
            expect_label(window, "result-summary", summary);
            expect_label(window, ("result-cell", 0_u64), "1");
            expect_label(window, ("result-cell", 50944_u64), "200");
            assert!(window.try_find(("result-row", 200_u64)).is_none());
        })
        .unwrap();
    }
}

async fn wait_status(cx: &mut TestAppContext, handle: AnyWindowHandle, status: &str) {
    cx.wait_for(handle, Duration::from_secs(10), |window, _| {
        window.find("connection-status").label() == Some(status)
    })
    .await;
}

async fn connect(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-database", cx);
        window.click("run-query", cx);
        expect_label(window, "connection-status", "Opening demo database…");
    })
    .unwrap();
    wait_status(cx, handle, "Demo database connected").await;
}

#[gpui_kit::test]
async fn keeps_busy_controls_inert_and_captured_sql_independent_of_edits_then_recovers_from_timeout(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    // A real aggregate cannot return a first row until its enormous recursion
    // finishes. The production SQLite progress deadline must interrupt it.
    run(
        cx,
        handle,
        "WITH RECURSIVE slow(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM slow WHERE n < 1000000000000) SELECT sum(n) FROM slow;",
    );
    let recovery_sql = "SELECT body FROM notes WHERE id = 1;";
    cx.update_window(handle, |_, window, cx| {
        window.click("run-query", cx);
        window.click("open-database", cx);
        expect_label(window, "connection-status", "Running query…");
        window.click("sql-editor", cx);
        window.press("secondary-a", cx);
        window.input(recovery_sql, cx);
        window.click("run-query", cx);
        window.click("theme-toggle", cx);
        assert_eq!(editor_value(window), recovery_sql);
        expect_label(window, "connection-status", "Running query…");
        expect_label(window, "database-path", path.to_str().unwrap());
        expect_label(window, "result-summary", "No query results");
    })
    .unwrap();
    wait_status(cx, handle, "Query timed out").await;
    cx.update_window(handle, |_, window, cx| {
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
        window.click("run-query", cx);
        expect_label(window, "connection-status", "Running query…");
    })
    .unwrap();
    wait_status(cx, handle, "Query complete: 1 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(
            window,
            ("result-cell", 0_u64),
            "\"Welcome to SQLite Workbench\"",
        );
    })
    .unwrap();
}

#[gpui_kit::test]
async fn reports_real_database_lock_and_keeps_the_connection_for_retry(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    run(cx, handle, "SELECT id FROM notes ORDER BY id;");
    wait_status(cx, handle, "Query complete: 2 rows").await;
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute_batch("BEGIN EXCLUSIVE;").unwrap();
    run(cx, handle, "SELECT id FROM notes ORDER BY id;");
    wait_status(cx, handle, "Database locked").await;
    cx.update_window(handle, |_, window, cx| {
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
        expect_label(window, "database-path", path.to_str().unwrap());
        window.click("open-database", cx);
        expect_label(window, "connection-status", "Database locked");
    })
    .unwrap();
    external.execute_batch("ROLLBACK;").unwrap();
    run(cx, handle, "SELECT id FROM notes ORDER BY id;");
    wait_status(cx, handle, "Query complete: 2 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, ("result-cell", 0_u64), "1");
        expect_label(window, ("result-cell", 256_u64), "2");
    })
    .unwrap();
}

fn run(cx: &mut TestAppContext, handle: AnyWindowHandle, sql: &str) {
    cx.update_window(handle, |_, window, cx| {
        window.click("sql-editor", cx);
        window.press("secondary-a", cx);
        window.input(sql, cx);
        window.click("run-query", cx);
        expect_label(window, "connection-status", "Running query…");
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
    })
    .unwrap();
}

fn expect_label(window: &Window, id: impl Into<ElementId>, expected: &str) {
    assert_eq!(window.find(id).label(), Some(expected));
}

fn editor_value(window: &Window) -> String {
    // Observe the real input's native value, not Workbench's private state.
    gpui_kit::base::test_support::snapshots(window)
        .into_iter()
        .find(|element| element.label() == Some("SQL editor"))
        .unwrap()
        .value()
        .unwrap()
        .to_owned()
}

#[gpui_kit::test]
async fn reads_notes_through_the_editor_and_preserves_results_across_theme_changes(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        expect_label(window, "run-query", "Run");
        window.click("run-query", cx);
        expect_label(window, "connection-status", "No database connected");
    })
    .unwrap();
    connect(cx, handle).await;
    cx.update_window(handle, |_, window, cx| {
        expect_label(window, "notes-preview", "Notes preview: 2 rows");
        window.click("run-query", cx);
        expect_label(window, "connection-status", "Running query…");
    })
    .unwrap();
    wait_status(cx, handle, "Query complete: 2 rows").await;
    cx.update_window(handle, |_, window, cx| {
        assert!(window.try_find("notes-preview").is_none());
        expect_label(window, ("result-column", 0_u64), "id");
        expect_label(window, ("result-column", 1_u64), "body");
        expect_label(window, ("result-cell", 0_u64), "1");
        expect_label(
            window,
            ("result-cell", 1_u64),
            "\"Welcome to SQLite Workbench\"",
        );
        expect_label(
            window,
            ("result-cell", 257_u64),
            "\"Your changes stay in this local file\"",
        );
        window.click("theme-toggle", cx);
        assert_eq!(
            editor_value(window),
            "SELECT id, body FROM notes ORDER BY id;"
        );
        expect_label(window, "connection-status", "Query complete: 2 rows");
        expect_label(window, "database-path", path.to_str().unwrap());
        expect_label(
            window,
            ("result-cell", 1_u64),
            "\"Welcome to SQLite Workbench\"",
        );
    })
    .unwrap();
    run(
        cx,
        handle,
        "SELECT body AS message FROM notes WHERE id = 2;",
    );
    wait_status(cx, handle, "Query complete: 1 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, ("result-column", 0_u64), "message");
        expect_label(
            window,
            ("result-cell", 0_u64),
            "\"Your changes stay in this local file\"",
        );
    })
    .unwrap();
}
