use crate::database::{CellValue, ReadResult};
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use std::fmt::Write;

pub(super) fn render(
    result: Option<&ReadResult>,
    outcome_summary: Option<&str>,
    cx: &App,
) -> impl IntoElement {
    let summary = match result {
        None => outcome_summary.unwrap_or("No query results").to_owned(),
        Some(result) if result.rows.is_empty() => "No rows returned".to_owned(),
        Some(result) => format!(
            "{} rows{}{}",
            result.rows.len(),
            if outcome_summary.is_some() {
                " returned"
            } else {
                ""
            },
            if result.truncated {
                " (truncated to 200 rows)"
            } else {
                ""
            }
        ),
    };
    let summary = match (outcome_summary, result) {
        (Some(outcome), Some(_)) => format!("{outcome}; {summary}"),
        _ => summary,
    };
    let mut panel = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .p_6()
        .gap_3()
        .child(
            div()
                .id("result-summary")
                .aria_label(summary.clone())
                .test_support()
                .child(summary),
        );
    if let Some(result) = result {
        let mut headers = div().id("result-headers").role(Role::Row).flex();
        for (index, column) in result.columns.iter().enumerate() {
            headers = headers.child(
                div()
                    .id(("result-column", index as u64))
                    .role(Role::ColumnHeader)
                    .aria_label(column.clone())
                    .test_support()
                    .w(px(160.0))
                    .flex_shrink_0()
                    .p_2()
                    .text_ellipsis()
                    .child(column.clone()),
            );
        }
        let mut table = div()
            .id("result-table")
            .role(Role::Table)
            .aria_label("Query results")
            .flex()
            .flex_col()
            .w(px(result.columns.len() as f32 * 160.0))
            .child(headers);
        for (row_index, row) in result.rows.iter().enumerate() {
            let mut rendered = div()
                .id(("result-row", row_index as u64))
                .role(Role::Row)
                .aria_label(format!("Result row {}", row_index + 1))
                .test_support()
                .flex()
                .border_b_1()
                .border_color(cx.theme().border);
            for (column_index, cell) in row.iter().enumerate() {
                let label = display(cell);
                // The read interface caps columns at 256, so these IDs remain
                // unique even for wide result sets.
                rendered = rendered.child(
                    div()
                        .id(("result-cell", (row_index * 256 + column_index) as u64))
                        .role(Role::Cell)
                        .aria_label(label.clone())
                        .test_support()
                        .w(px(160.0))
                        .flex_shrink_0()
                        .p_2()
                        .text_ellipsis()
                        .child(if matches!(cell, CellValue::Null) {
                            "(SQL NULL)".to_owned()
                        } else {
                            label
                        }),
                );
            }
            table = table.child(rendered);
        }
        panel = panel.child(
            div()
                .id("results-scroll")
                .flex_1()
                .min_w_0()
                .min_h_0()
                .overflow_x_scroll()
                .overflow_y_scroll()
                .child(table),
        );
    }
    panel
}

fn display(cell: &CellValue) -> String {
    match cell {
        CellValue::Null => "SQL NULL".to_owned(),
        CellValue::Integer(value) => value.to_string(),
        CellValue::Real(value) => value.to_string(),
        CellValue::Text(value) => format!("{value:?}"),
        CellValue::Blob(bytes) => {
            let mut result = String::with_capacity(bytes.len() * 2 + 3);
            result.push_str("X'");
            for byte in bytes {
                write!(result, "{byte:02X}").expect("writing to String");
            }
            result.push('\'');
            result
        }
    }
}
