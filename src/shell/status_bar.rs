use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

pub(super) fn render(status: SharedString, cx: &App) -> Div {
    let theme = cx.theme();

    div()
        .flex()
        .items_center()
        .justify_between()
        .px_5()
        .py_2()
        .border_t_1()
        .border_color(theme.border)
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(
            div()
                .id("connection-status")
                .role(Role::Status)
                .aria_label(status.clone())
                .test_support()
                .child(status),
        )
        .child("GPUI Kit · Starter")
}
