use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

pub(super) fn render(cx: &App) -> Div {
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
                .child("No databases yet")
                .child("Tables and views will appear here."),
        )
}
