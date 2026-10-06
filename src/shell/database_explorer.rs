use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use std::path::Path;

pub(super) fn render(path: Option<&Path>, cx: &App) -> Div {
    let theme = cx.theme();

    div()
        .flex()
        .flex_col()
        .w(px(240.0))
        .flex_shrink_0()
        .p_5()
        .gap_4()
        .border_r_1()
        .border_color(theme.border)
        .child(div().text_sm().child("DATABASE EXPLORER"))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(match path {
                    Some(path) => div()
                        .id("database-path")
                        .aria_label(path.display().to_string())
                        .test_support()
                        .child(path.display().to_string())
                        .into_any_element(),
                    None => div().child("No databases yet").into_any_element(),
                })
                .child(match path {
                    Some(_) => div()
                        .id("notes-table")
                        .aria_label("Table: notes")
                        .test_support()
                        .child("notes")
                        .into_any_element(),
                    None => div()
                        .child("Open the demo to explore notes.")
                        .into_any_element(),
                }),
        )
}
