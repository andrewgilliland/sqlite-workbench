use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

pub(super) fn render(cx: &App) -> Div {
    let theme = cx.theme();

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
                .text_color(theme.muted_foreground)
                .child("A native workspace for exploring your data."),
        )
        .child(
            div()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Shell ready. Database connections are coming next."),
        )
}
