#[path = "../src/shell.rs"]
mod shell;

use gpui_kit::component::{ActiveTheme, Theme, ThemeMode};
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size};

#[gpui_kit::test]
fn switches_theme_through_the_ui_without_losing_status(cx: &mut TestAppContext) {
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
            |_, cx| cx.new(|_| shell::Workbench::default()),
        )
        .expect("open workbench test window");
        handle
    });

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(!cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").label(), Some("Dark mode"));
        assert_eq!(window.find("theme-toggle").checked(), Some(false));

        window.click("open-database", cx);
        let status = "Opening databases is coming next. No database connected.";
        assert_eq!(window.find("connection-status").label(), Some(status));

        window.click("theme-toggle", cx);
        assert!(cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").label(), Some("Dark mode"));
        assert_eq!(window.find("theme-toggle").checked(), Some(true));
        assert_eq!(window.find("connection-status").label(), Some(status));

        window.click("theme-toggle", cx);
        assert!(!cx.theme().is_dark());
        assert_eq!(window.find("theme-toggle").label(), Some("Dark mode"));
        assert_eq!(window.find("theme-toggle").checked(), Some(false));
        assert_eq!(window.find("connection-status").label(), Some(status));
    })
    .unwrap();
}
