use gpui_kit::component::{
    ActiveTheme, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    switch::Switch,
};
use gpui_kit::*;

#[derive(Default)]
pub struct Workbench {
    open_requested: bool,
}

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let background = theme.background;
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let border = theme.border;
        let is_dark = theme.is_dark();
        let status = if self.open_requested {
            "Opening databases is coming next. No database connected."
        } else {
            "No database connected"
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_6()
                    .py_4()
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_lg().child("SQLite Workbench"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(muted)
                                    .child("Your local data, a little clearer."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Switch::new("theme-toggle")
                                    .label(if is_dark { "Dark mode" } else { "Light mode" })
                                    .checked(is_dark)
                                    .on_change(|checked, window, cx| {
                                        let mode = if *checked {
                                            ThemeMode::Dark
                                        } else {
                                            ThemeMode::Light
                                        };
                                        Theme::change(mode, Some(window), cx);
                                    }),
                            )
                            .child(
                                Button::new("open-database")
                                    .primary()
                                    .label("Open Database")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_requested = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w(px(240.0))
                            .flex_shrink_0()
                            .p_5()
                            .gap_4()
                            .border_r_1()
                            .border_color(border)
                            .child(div().text_sm().child("DATABASE EXPLORER"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .text_sm()
                                    .text_color(muted)
                                    .child("No databases yet")
                                    .child("Tables and views will appear here."),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .p_8()
                            .gap_4()
                            .child(div().text_3xl().child("Hello, SQLite!"))
                            .child(
                                div()
                                    .text_color(muted)
                                    .child("A native workspace for exploring your data."),
                            )
                            .child(
                                div()
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(border)
                                    .text_sm()
                                    .text_color(muted)
                                    .child("Shell ready. Database connections are coming next."),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_5()
                    .py_2()
                    .border_t_1()
                    .border_color(border)
                    .text_xs()
                    .text_color(muted)
                    .child(
                        div()
                            .id("connection-status")
                            .role(Role::Status)
                            .aria_label(status)
                            .test_support()
                            .child(status),
                    )
                    .child("GPUI Kit · Starter"),
            )
    }
}
