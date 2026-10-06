use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px,
    size,
};
use sqlite_workbench::{database::DemoSession, shell::Workbench};
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
async fn null_and_literal_null_are_distinct_in_the_preview(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    drop(DemoSession::open(&path).unwrap());
    let external = rusqlite::Connection::open(&path).unwrap();
    external
        .execute_batch("DELETE FROM notes; INSERT INTO notes VALUES (1, NULL), (2, 'NULL');")
        .unwrap();
    let handle = open_window(cx, path);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-database", cx);
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(5), |window, _| {
        window.find("connection-status").label() == Some("Demo database connected")
    })
    .await;
    cx.update_window(handle, |_, window, _| {
        assert_eq!(
            window.find(("note", 1_u64)).label(),
            Some("Note 1: SQL NULL")
        );
        assert_eq!(window.find(("note", 2_u64)).label(), Some("Note 2: NULL"));
    })
    .unwrap();
}

#[gpui_kit::test]
async fn opens_real_notes_and_ignores_duplicate_opening(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("open-database").label(),
            Some("Open Demo Database")
        );
        assert_eq!(
            window.find("connection-status").label(),
            Some("No database connected")
        );
        assert!(window.try_find("database-path").is_none());
        window.click("open-database", cx);
        assert_eq!(
            window.find("connection-status").label(),
            Some("Opening demo database…")
        );
        window.click("open-database", cx);
        assert_eq!(
            window.find("connection-status").label(),
            Some("Opening demo database…")
        );
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(5), |window, _| {
        window.find("connection-status").label() == Some("Demo database connected")
    })
    .await;
    cx.update_window(handle, |_, window, cx| {
        assert_eq!(
            window.find("database-path").label(),
            Some(path.to_str().unwrap())
        );
        assert_eq!(window.find("notes-table").label(), Some("Table: notes"));
        assert_eq!(
            window.find(("note", 1_u64)).label(),
            Some("Note 1: Welcome to SQLite Workbench")
        );
        assert_eq!(
            window.find(("note", 2_u64)).label(),
            Some("Note 2: Your changes stay in this local file")
        );
        window.click("open-database", cx);
        assert_eq!(
            window.find("connection-status").label(),
            Some("Demo database connected")
        );
    })
    .unwrap();
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .preview()
            .unwrap()
            .notes
            .len(),
        2
    );
}

#[gpui_kit::test]
async fn reopening_shows_external_data_and_preserves_empty_notes(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    drop(DemoSession::open(&path).unwrap());
    let external = rusqlite::Connection::open(&path).unwrap();
    external
        .execute(
            "UPDATE notes SET body = 'Persisted outside the app' WHERE id = 1",
            [],
        )
        .unwrap();
    let first = open_window(cx, path.clone());
    cx.update_window(first, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-database", cx);
    })
    .unwrap();
    cx.wait_for(first, Duration::from_secs(5), |window, _| {
        window.find("connection-status").label() == Some("Demo database connected")
    })
    .await;
    cx.update_window(first, |_, window, cx| {
        assert_eq!(
            window.find(("note", 1_u64)).label(),
            Some("Note 1: Persisted outside the app")
        );
        window.remove_window();
        cx.refresh_windows();
    })
    .unwrap();
    external.execute("DELETE FROM notes", []).unwrap();
    let second = open_window(cx, path);
    cx.update_window(second, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-database", cx);
    })
    .unwrap();
    cx.wait_for(second, Duration::from_secs(5), |window, _| {
        window.find("connection-status").label() == Some("Demo database connected")
    })
    .await;
    cx.update_window(second, |_, window, _| {
        assert_eq!(
            window.find("notes-preview").label(),
            Some("Notes preview: 0 rows")
        );
        assert_eq!(window.find("notes-empty").label(), Some("No notes yet"));
    })
    .unwrap();
}

#[gpui_kit::test]
async fn opening_failure_is_truthful_and_allows_retry(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    std::fs::write(&path, b"invalid database").unwrap();
    let handle = open_window(cx, path.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("open-database", cx);
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(5), |window, _| {
        window
            .find("connection-status")
            .label()
            .is_some_and(|label| label.starts_with("Could not open demo database:"))
    })
    .await;
    cx.update_window(handle, |_, window, _| {
        assert!(window.try_find("database-path").is_none());
        assert!(window.try_find("notes-preview").is_none());
    })
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"invalid database");
    // Only the test owner removes its invalid fixture; the app must never repair by replacement.
    std::fs::remove_file(&path).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.click("open-database", cx);
    })
    .unwrap();
    cx.wait_for(handle, Duration::from_secs(5), |window, _| {
        window.find("connection-status").label() == Some("Demo database connected")
    })
    .await;
}
