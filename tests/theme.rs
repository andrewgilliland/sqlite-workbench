use gpui_kit::component::{ActiveTheme, Theme, ThemeMode};
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};
use sqlite_workbench::shell::Workbench;
use std::time::Duration;

#[gpui_kit::test]
async fn switches_theme_through_the_ui_without_losing_status(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let handle = cx.update(|cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Light, None, cx);
        let (handle, _) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(1100.0), px(720.0)),
                })),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|_| Workbench::with_database_path(path.clone())),
        )
        .expect("open workbench test window");
        handle
    });

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(!cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").label(), Some("Light mode"));
        assert_eq!(window.find("theme-toggle").checked(), Some(false));

        window.click("open-database", cx);
        let status = "Opening demo database…";
        assert_eq!(window.find("connection-status").label(), Some(status));

        window.click("theme-toggle", cx);
        assert!(cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").label(), Some("Dark mode"));
        assert_eq!(window.find("theme-toggle").checked(), Some(true));
        assert_eq!(window.find("connection-status").label(), Some(status));

        window.click("theme-toggle", cx);
        assert!(!cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").label(), Some("Light mode"));
        assert_eq!(window.find("theme-toggle").checked(), Some(false));
        assert_eq!(window.find("connection-status").label(), Some(status));
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
        assert_eq!(
            window.find("notes-preview").label(),
            Some("Notes preview: 2 rows")
        );
        window.click("theme-toggle", cx);
        assert!(cx.theme().is_dark());
        assert_eq!(
            window.find("connection-status").label(),
            Some("Demo database connected")
        );
        assert_eq!(
            window.find("notes-preview").label(),
            Some("Notes preview: 2 rows")
        );
        window.click("open-database", cx);
        assert_eq!(
            window.find("connection-status").label(),
            Some("Demo database connected")
        );
    })
    .unwrap();
}
