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
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
    })
    .unwrap();
}

fn expect_label(window: &Window, id: impl Into<ElementId>, expected: &str) {
    assert_eq!(window.find(id).label(), Some(expected));
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

async fn write(cx: &mut TestAppContext, handle: AnyWindowHandle, sql: &str, count: usize) {
    run(cx, handle, sql);
    let status = format!("Write committed: {count} rows affected");
    wait_status(cx, handle, &status).await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, "result-summary", &status);
        assert!(window.try_find("result-table").is_none());
        assert!(window.try_find(("result-column", 0_u64)).is_none());
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
    })
    .unwrap();
}

async fn read_cell(cx: &mut TestAppContext, handle: AnyWindowHandle, sql: &str, value: &str) {
    run(cx, handle, sql);
    wait_status(cx, handle, "Query complete: 1 rows").await;
    cx.update_window(handle, |_, window, _| {
        expect_label(window, "result-summary", "1 rows");
        expect_label(window, ("result-cell", 0_u64), value);
    })
    .unwrap();
}

#[gpui_kit::test]
async fn commits_crud_and_zero_counts_with_readback_and_theme_preserved(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    read_cell(cx, handle, "SELECT count(*) FROM notes;", "2").await;
    let insert = "INSERT INTO notes(body) VALUES ('UI persistence');";
    write(cx, handle, insert, 1).await;
    cx.update_window(handle, |_, window, cx| {
        let was_dark = cx.theme().is_dark();
        window.click("theme-toggle", cx);
        assert_ne!(cx.theme().is_dark(), was_dark);
        assert_eq!(editor_value(window), insert);
        expect_label(
            window,
            "connection-status",
            "Write committed: 1 rows affected",
        );
        expect_label(window, "result-summary", "Write committed: 1 rows affected");
        expect_label(window, "database-path", path.to_str().unwrap());
        window.click("open-database", cx);
        expect_label(
            window,
            "connection-status",
            "Write committed: 1 rows affected",
        );
    })
    .unwrap();
    read_cell(
        cx,
        handle,
        "SELECT body FROM notes WHERE id=3;",
        "\"UI persistence\"",
    )
    .await;
    write(
        cx,
        handle,
        "UPDATE notes SET body='UI verified' WHERE id=3;",
        1,
    )
    .await;
    read_cell(
        cx,
        handle,
        "SELECT body FROM notes WHERE id=3;",
        "\"UI verified\"",
    )
    .await;
    write(
        cx,
        handle,
        "UPDATE notes SET body='absent' WHERE id=999;",
        0,
    )
    .await;
    write(cx, handle, "DELETE FROM notes WHERE id=999;", 0).await;
    write(cx, handle, "DELETE FROM notes WHERE id=3;", 1).await;
    read_cell(cx, handle, "SELECT count(*) FROM notes WHERE id=3;", "0").await;
    read_cell(cx, handle, "SELECT count(*) FROM notes;", "2").await;
}

fn close_window(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| {
        window.remove_window();
        cx.refresh_windows();
    })
    .unwrap();
}

#[gpui_kit::test]
async fn closing_and_reopening_workbench_preserves_writes_and_never_reseeds_empty_notes(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let first = open_window(cx, path.clone());
    connect(cx, first).await;
    write(
        cx,
        first,
        "INSERT INTO notes(body) VALUES ('restart verified');",
        1,
    )
    .await;
    write(
        cx,
        first,
        "UPDATE notes SET body='updated on disk' WHERE id=3;",
        1,
    )
    .await;
    close_window(cx, first);

    let second = open_window(cx, path.clone());
    connect(cx, second).await;
    cx.update_window(second, |_, window, _| {
        expect_label(window, "notes-preview", "Notes preview: 3 rows");
        expect_label(window, ("note", 3_u64), "Note 3: updated on disk");
    })
    .unwrap();
    read_cell(
        cx,
        second,
        "SELECT body FROM notes WHERE id=3;",
        "\"updated on disk\"",
    )
    .await;
    write(cx, second, "DELETE FROM notes;", 3).await;
    close_window(cx, second);

    let third = open_window(cx, path);
    connect(cx, third).await;
    cx.update_window(third, |_, window, _| {
        expect_label(window, "notes-preview", "Notes preview: 0 rows");
        expect_label(window, "notes-empty", "No notes yet");
    })
    .unwrap();
    read_cell(cx, third, "SELECT count(*) FROM notes;", "0").await;
    write(cx, third, "DELETE FROM notes;", 0).await;
    write(
        cx,
        third,
        "INSERT INTO notes(body) VALUES ('fresh after empty');",
        1,
    )
    .await;
    read_cell(
        cx,
        third,
        "SELECT body FROM notes;",
        "\"fresh after empty\"",
    )
    .await;
}

#[gpui_kit::test]
async fn forbidden_sql_scripts_and_partial_constraint_failure_clear_outcomes_and_allow_retry(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    for (sql, category) in [
        ("PRAGMA user_version=7;", "Unsupported SQL:"),
        (
            "CREATE TABLE forbidden (id INTEGER PRIMARY KEY);",
            "Unsupported SQL:",
        ),
        ("BEGIN;", "Unsupported SQL:"),
        (
            "INSERT INTO notes(body) VALUES ('forbidden'); DELETE FROM notes;",
            "Unsupported SQL:",
        ),
        // OR FAIL would preserve the first inserted row without the owning rollback.
        (
            "INSERT OR FAIL INTO notes(id, body) VALUES (100, 'partial'), (1, 'collision');",
            "Query error:",
        ),
    ] {
        write(cx, handle, "UPDATE notes SET body=body WHERE id=1;", 1).await;
        run(cx, handle, sql);
        cx.wait_for(handle, Duration::from_secs(15), |window, _| {
            window
                .find("connection-status")
                .label()
                .is_some_and(|label| label.starts_with(category))
        })
        .await;
        cx.update_window(handle, |_, window, cx| {
            expect_label(window, "result-summary", "No query results");
            assert!(window.try_find(("result-column", 0_u64)).is_none());
            assert!(window.try_find(("result-cell", 0_u64)).is_none());
            expect_label(window, "database-path", path.to_str().unwrap());
            let status = window.find("connection-status").label().unwrap().to_owned();
            window.click("open-database", cx);
            expect_label(window, "connection-status", &status);
        })
        .unwrap();
        let external = rusqlite::Connection::open(&path).unwrap();
        let user_version: i64 = external
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_version, 0);
        let forbidden_tables: i64 = external
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name='forbidden'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(forbidden_tables, 0);
        read_cell(cx, handle, "SELECT count(*) FROM notes;", "2").await;
        read_cell(
            cx,
            handle,
            "SELECT body FROM notes WHERE id=1;",
            "\"Welcome to SQLite Workbench\"",
        )
        .await;
        read_cell(
            cx,
            handle,
            "SELECT body FROM notes WHERE id=2;",
            "\"Your changes stay in this local file\"",
        )
        .await;
        write(
            cx,
            handle,
            "INSERT INTO notes(id, body) VALUES (100, 'retry');",
            1,
        )
        .await;
        read_cell(
            cx,
            handle,
            "SELECT body FROM notes WHERE id=100;",
            "\"retry\"",
        )
        .await;
        write(cx, handle, "DELETE FROM notes WHERE id=100;", 1).await;
    }
}

#[gpui_kit::test]
async fn real_insert_deadline_rolls_back_and_busy_controls_never_queue_edited_sql(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    write(cx, handle, "UPDATE notes SET body=body WHERE id=1;", 1).await;
    let started = std::time::Instant::now();
    run(
        cx,
        handle,
        "WITH RECURSIVE slow(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM slow WHERE n<1000000000000) INSERT INTO notes(body) SELECT 'deadline rollback' FROM slow;",
    );
    let recovery_sql = "INSERT INTO notes(body) VALUES ('captured retry');";
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..3 {
            window.click("run-query", cx);
            window.click("open-database", cx);
        }
        window.click("sql-editor", cx);
        window.press("secondary-a", cx);
        window.input(recovery_sql, cx);
        for _ in 0..3 {
            window.click("run-query", cx);
            window.click("open-database", cx);
        }
        let was_dark = cx.theme().is_dark();
        window.click("theme-toggle", cx);
        assert_ne!(cx.theme().is_dark(), was_dark);
        assert_eq!(editor_value(window), recovery_sql);
        expect_label(window, "connection-status", "Running query…");
        expect_label(window, "result-summary", "No query results");
        expect_label(window, "database-path", path.to_str().unwrap());
    })
    .unwrap();
    wait_status(cx, handle, "Query timed out").await;
    assert!(started.elapsed() >= Duration::from_secs(5));
    cx.update_window(handle, |_, window, _| {
        assert_eq!(editor_value(window), recovery_sql);
        expect_label(window, "result-summary", "No query results");
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
    })
    .unwrap();
    // The edited statement must not have been queued, and the huge INSERT must
    // have left neither partial rows nor a late duplicate execution behind.
    read_cell(cx, handle, "SELECT count(*) FROM notes;", "2").await;
    read_cell(
        cx,
        handle,
        "SELECT count(*) FROM notes WHERE body='captured retry';",
        "0",
    )
    .await;
    write(cx, handle, recovery_sql, 1).await;
    read_cell(cx, handle, "SELECT count(*) FROM notes;", "3").await;
    close_window(cx, handle);
    let reopened = open_window(cx, path);
    connect(cx, reopened).await;
    read_cell(cx, reopened, "SELECT count(*) FROM notes;", "3").await;
    read_cell(
        cx,
        reopened,
        "SELECT count(*) FROM notes WHERE body='captured retry';",
        "1",
    )
    .await;
}

#[gpui_kit::test]
async fn failed_commit_under_external_read_lock_reports_no_success_and_allows_retry(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    connect(cx, handle).await;
    write(cx, handle, "UPDATE notes SET body=body WHERE id=1;", 1).await;
    let external = rusqlite::Connection::open(&path).unwrap();
    // A held SHARED lock allows the writer to step but prevents its COMMIT
    // acquiring EXCLUSIVE. An external BEGIN EXCLUSIVE would only test entry.
    external
        .execute_batch("BEGIN; SELECT id FROM notes;")
        .unwrap();
    let insert = "INSERT INTO notes(id, body) VALUES (100, 'commit retry');";
    run(cx, handle, insert);
    wait_status(cx, handle, "Database locked").await;
    cx.update_window(handle, |_, window, cx| {
        expect_label(window, "result-summary", "No query results");
        assert!(window.try_find(("result-column", 0_u64)).is_none());
        assert!(window.try_find(("result-cell", 0_u64)).is_none());
        expect_label(window, "database-path", path.to_str().unwrap());
        let was_dark = cx.theme().is_dark();
        window.click("theme-toggle", cx);
        assert_ne!(cx.theme().is_dark(), was_dark);
        assert_eq!(editor_value(window), insert);
        expect_label(window, "connection-status", "Database locked");
        window.click("open-database", cx);
        expect_label(window, "connection-status", "Database locked");
    })
    .unwrap();
    read_cell(cx, handle, "SELECT count(*) FROM notes;", "2").await;
    read_cell(cx, handle, "SELECT count(*) FROM notes WHERE id=100;", "0").await;
    external.execute_batch("ROLLBACK;").unwrap();
    write(cx, handle, insert, 1).await;
    read_cell(
        cx,
        handle,
        "SELECT body FROM notes WHERE id=100;",
        "\"commit retry\"",
    )
    .await;
    read_cell(cx, handle, "SELECT count(*) FROM notes;", "3").await;
}
