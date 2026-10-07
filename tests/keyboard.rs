use gpui_kit::component::{ActiveTheme, Theme, ThemeMode};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Point, Role, TestAppContext, Window, WindowBounds,
    WindowOptions, px, size,
};
use sqlite_workbench::shell::Workbench;
use std::{path::PathBuf, time::Duration};

fn open_window(cx: &mut TestAppContext, path: PathBuf) -> AnyWindowHandle {
    let handle = cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
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
        .expect("open production Workbench")
        .0
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    handle
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, key: &str) {
    cx.update_window(handle, |_, window, cx| window.press(key, cx))
        .unwrap();
    // Match the installed GPUI input-focus tests: settle deferred focus
    // subscriptions before inspecting snapshots or sending the next event.
    cx.run_until_parked();
}

fn expect_focus(window: &Window, label: &str) {
    let snapshots = gpui_kit::base::test_support::snapshots(window);
    let element = snapshots
        .iter()
        .find(|element| element.label() == Some(label))
        .unwrap_or_else(|| panic!("missing native control labeled {label:?}"));
    let focused_labels: Vec<_> = snapshots
        .iter()
        .filter(|element| element.focused() == Some(true))
        .map(|element| element.label())
        .collect();
    assert_eq!(
        element.focused(),
        Some(true),
        "expected keyboard focus on {label:?}; focused native labels: {focused_labels:?}"
    );
}

fn expect_editor(window: &Window, sql: &str) {
    let snapshots = gpui_kit::base::test_support::snapshots(window);
    let editor = snapshots
        .iter()
        .find(|element| element.label() == Some("SQL editor"))
        .expect("meaningfully labeled native SQL editor");
    assert_eq!(editor.value(), Some(sql));
}

#[gpui_kit::test]
async fn keyboard_traversal_opens_edits_runs_and_preserves_results_across_theme_changes(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = open_window(cx, path.clone());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(!cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").label(), Some("Light mode"));
        assert_eq!(window.find("theme-toggle").checked(), Some(false));
        assert_eq!(
            window.find("open-database").label(),
            Some("Open Demo Database")
        );
        assert_eq!(window.find("run-query").label(), Some("Run"));
        assert_eq!(window.find("connection-status").role(), Some(Role::Status));
        assert_eq!(
            window.find("connection-status").label(),
            Some("No database connected")
        );
        expect_editor(window, "SELECT id, body FROM notes ORDER BY id;");
        assert!(window.try_find("database-path").is_none());
    })
    .unwrap();
    assert!(
        !path.exists(),
        "rendering must not open or create a database"
    );

    // No click, focus_next/focus_prev, extra binding, or dispatched action:
    // even the first tab stop must be reachable by a real keyboard event.
    press(cx, handle, "tab");
    cx.update_window(handle, |_, window, _| expect_focus(window, "Light mode"))
        .unwrap();
    press(cx, handle, "space");
    cx.update_window(handle, |_, window, cx| {
        expect_focus(window, "Dark mode");
        assert!(cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").checked(), Some(true));
        assert_eq!(
            window.find("connection-status").label(),
            Some("No database connected")
        );
    })
    .unwrap();
    press(cx, handle, "enter");
    cx.update_window(handle, |_, window, cx| {
        expect_focus(window, "Light mode");
        assert!(!cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").checked(), Some(false));
    })
    .unwrap();

    press(cx, handle, "tab");
    cx.update_window(handle, |_, window, _| {
        expect_focus(window, "Open Demo Database");
    })
    .unwrap();
    press(cx, handle, "shift-tab");
    cx.update_window(handle, |_, window, _| expect_focus(window, "Light mode"))
        .unwrap();
    press(cx, handle, "tab");
    cx.update_window(handle, |_, window, _| {
        expect_focus(window, "Open Demo Database");
    })
    .unwrap();
    press(cx, handle, "enter");
    cx.wait_for(handle, Duration::from_secs(10), |window, _| {
        window.find("connection-status").label() == Some("Demo database connected")
    })
    .await;
    cx.update_window(handle, |_, window, _| {
        assert_eq!(
            window.find("database-path").label(),
            Some(path.to_str().unwrap())
        );
        assert_eq!(window.find("notes-table").label(), Some("Table: notes"));
        assert_eq!(
            window.find("notes-preview").label(),
            Some("Notes preview: 2 rows")
        );
    })
    .unwrap();
    assert!(
        path.is_file(),
        "keyboard activation must open the isolated file"
    );

    cx.update_window(handle, |_, window, _| expect_focus(window, "SQL editor"))
        .unwrap();
    let sql = "SELECT body AS keyboard_note FROM notes WHERE id = 1;";
    press(cx, handle, "secondary-a");
    cx.update_window(handle, |_, window, cx| window.input(sql, cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, _| {
        expect_focus(window, "SQL editor");
        expect_editor(window, sql);
    })
    .unwrap();

    // Editor Tab remains indentation. The documented control-navigation shortcut
    // crosses the same production key event path rather than setting focus in tests.
    press(cx, handle, "ctrl-tab");
    cx.update_window(handle, |_, window, _| {
        expect_focus(window, "Run");
        expect_editor(window, sql);
    })
    .unwrap();
    press(cx, handle, "ctrl-shift-tab");
    cx.update_window(handle, |_, window, _| {
        expect_focus(window, "SQL editor");
        expect_editor(window, sql);
    })
    .unwrap();
    press(cx, handle, "ctrl-tab");
    press(cx, handle, "space");
    cx.wait_for(handle, Duration::from_secs(10), |window, _| {
        window.find("connection-status").label() == Some("Query complete: 1 rows")
    })
    .await;
    cx.update_window(handle, |_, window, _| {
        assert_eq!(window.find("connection-status").role(), Some(Role::Status));
        assert_eq!(window.find("result-summary").label(), Some("1 rows"));
        assert_eq!(
            window.find(("result-column", 0_u64)).label(),
            Some("keyboard_note")
        );
        assert_eq!(
            window.find(("result-cell", 0_u64)).label(),
            Some("\"Welcome to SQLite Workbench\"")
        );
    })
    .unwrap();

    cx.update_window(handle, |_, window, _| expect_focus(window, "SQL editor"))
        .unwrap();
    press(cx, handle, "secondary-enter");
    cx.wait_for(handle, Duration::from_secs(10), |window, _| {
        window.find("connection-status").label() == Some("Query complete: 1 rows")
    })
    .await;
    cx.update_window(handle, |_, window, _| expect_editor(window, sql))
        .unwrap();
    press(cx, handle, "ctrl-tab");

    // Noninteractive results/status must not swallow the next tab stop.
    press(cx, handle, "tab");
    cx.update_window(handle, |_, window, _| expect_focus(window, "Light mode"))
        .unwrap();
    press(cx, handle, "space");
    cx.update_window(handle, |_, window, cx| {
        expect_focus(window, "Dark mode");
        assert!(cx.theme().is_dark());
        expect_editor(window, sql);
        assert_eq!(
            window.find("database-path").label(),
            Some(path.to_str().unwrap())
        );
        assert_eq!(
            window.find("connection-status").label(),
            Some("Query complete: 1 rows")
        );
        assert_eq!(
            window.find(("result-cell", 0_u64)).label(),
            Some("\"Welcome to SQLite Workbench\"")
        );
    })
    .unwrap();
}
