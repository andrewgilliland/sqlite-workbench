use gpui_kit::component::Theme;
use gpui_kit::*;
use sqlite_workbench::shell;

fn main() {
    let mut arguments = std::env::args_os().skip(1);
    let database_path = match arguments.next() {
        None => None,
        Some(flag) if flag == "--demo-data-dir" => {
            let Some(directory) = arguments.next() else {
                eprintln!("Usage: sqlite-workbench [--demo-data-dir ABSOLUTE_DIRECTORY]");
                std::process::exit(2);
            };
            match sqlite_workbench::database::demo_path_in(std::path::Path::new(&directory)) {
                Ok(path) => Some(path),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            }
        }
        Some(_) => {
            eprintln!("Usage: sqlite-workbench [--demo-data-dir ABSOLUTE_DIRECTORY]");
            std::process::exit(2);
        }
    };
    if arguments.next().is_some() {
        eprintln!("Usage: sqlite-workbench [--demo-data-dir ABSOLUTE_DIRECTORY]");
        std::process::exit(2);
    }
    application().with_assets(assets::Assets).run(move |cx| {
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
            move |window, cx| {
                window.set_window_title("SQLite Workbench");
                cx.new(|_| match database_path {
                    Some(path) => shell::Workbench::with_database_path(path),
                    None => shell::Workbench::default(),
                })
            },
        )
        .expect("Failed to open SQLite Workbench window");

        cx.activate(true);
    });
}
