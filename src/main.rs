use gpui_kit::component::Theme;
use gpui_kit::*;
use sqlite_workbench::shell;

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        Theme::sync_system_appearance(None, cx);

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(720.0)), cx);
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(760.0), px(480.0))),
                ..Default::default()
            },
            cx,
            |window, cx| {
                window.set_window_title("SQLite Workbench");
                cx.new(|_| shell::Workbench::default())
            },
        )
        .expect("Failed to open SQLite Workbench window");

        cx.activate(true);
    });
}
