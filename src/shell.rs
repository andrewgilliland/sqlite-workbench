#[path = "shell/database_explorer.rs"]
mod database_explorer;
#[path = "shell/header.rs"]
mod header;
#[path = "shell/status_bar.rs"]
mod status_bar;
#[path = "shell/welcome_panel.rs"]
mod welcome_panel;

use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

#[derive(Default)]
pub struct Workbench {
    open_requested: bool,
}

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_open_database = cx.listener(|this, _, _, cx| {
            this.open_requested = true;
            cx.notify();
        });
        let status = if self.open_requested {
            "Opening databases is coming next. No database connected."
        } else {
            "No database connected"
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(header::render(cx, on_open_database))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(database_explorer::render(cx))
                    .child(welcome_panel::render(cx)),
            )
            .child(status_bar::render(status, cx))
    }
}
