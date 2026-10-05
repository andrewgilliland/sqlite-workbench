use gpui_kit::component::{
    ActiveTheme, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    switch::Switch,
};
use gpui_kit::*;

pub(super) fn render(
    cx: &App,
    on_open_database: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let theme = cx.theme();
    let is_dark = theme.is_dark();

    div()
        .flex()
        .items_center()
        .justify_between()
        .px_6()
        .py_4()
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_lg().child("SQLite Workbench"))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
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
                        .on_click(on_open_database),
                ),
        )
}
