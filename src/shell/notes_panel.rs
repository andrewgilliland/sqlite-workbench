use crate::database::NotesPreview;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

pub(super) fn render(preview: &NotesPreview, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    let mut rows = div().flex().flex_col().gap_2();
    if preview.notes.is_empty() {
        rows = rows.child(
            div()
                .id("notes-empty")
                .aria_label("No notes yet")
                .test_support()
                .text_color(theme.muted_foreground)
                .child("No notes yet"),
        );
    }
    for note in &preview.notes {
        let accessible_body = note.body.as_deref().unwrap_or("SQL NULL");
        let body = match &note.body {
            Some(body) => div().child(body.clone()),
            None => div().text_color(theme.muted_foreground).child("(SQL NULL)"),
        };
        rows = rows.child(
            div()
                .id(("note", note.id as u64))
                .aria_label(format!("Note {}: {accessible_body}", note.id))
                .test_support()
                .flex()
                .gap_4()
                .p_3()
                .border_b_1()
                .border_color(theme.border)
                .child(div().w(px(60.0)).flex_shrink_0().child(note.id.to_string()))
                .child(body),
        );
    }
    div()
        .id("notes-preview")
        .aria_label(format!("Notes preview: {} rows", preview.notes.len()))
        .test_support()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .p_6()
        .gap_4()
        .child(div().text_lg().child("Notes preview"))
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Read from your local file · SQL editing is coming next"),
        )
        .child(
            div()
                .flex()
                .gap_4()
                .child(div().w(px(60.0)).child("id"))
                .child("body"),
        )
        .child(
            div()
                .id("notes-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(rows),
        )
        .child(if preview.truncated {
            "Showing the first 200 notes; additional notes are not displayed."
        } else {
            ""
        })
}
