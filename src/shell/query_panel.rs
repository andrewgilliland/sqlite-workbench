use gpui_kit::component::{
    ActiveTheme, Disableable,
    button::{Button, ButtonVariants},
    input::{Editor, EditorState},
};
use gpui_kit::*;

pub(super) fn render(
    editor: &Entity<EditorState>,
    run_disabled: bool,
    on_run: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    content: AnyElement,
    cx: &App,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .child(
            div()
                .p_6()
                .border_b_1()
                .border_color(cx.theme().border)
                .flex()
                .flex_col()
                .gap_3()
                .child(div().text_lg().child("SQL query"))
                .child(div().text_sm().text_color(cx.theme().muted_foreground)
                    .child("Cmd/Ctrl+Enter: Run · Ctrl+Tab / Ctrl+Shift+Tab: move between controls"))
                // Editor has no custom ID builder in 0.7.0. Keep a stable
                // region ID; the inner input retains its own native identity.
                .child(
                    div()
                        .id("sql-editor")
                        .test_support()
                        .h(px(180.0))
                        .child(Editor::new(editor).aria_label("SQL editor").h(px(180.0))),
                )
                .child(
                    div().flex().justify_end().child(
                        Button::new("run-query")
                            .primary()
                            .label("Run")
                            .disabled(run_disabled)
                            .on_click(on_run),
                    ),
                ),
        )
        .child(content)
}
