#[path = "shell/database_explorer.rs"]
mod database_explorer;
#[path = "shell/header.rs"]
mod header;
#[path = "shell/notes_panel.rs"]
mod notes_panel;
#[path = "shell/status_bar.rs"]
mod status_bar;
#[path = "shell/welcome_panel.rs"]
mod welcome_panel;

use crate::database::{DemoSession, NotesPreview, demo_path};
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use std::path::PathBuf;

pub struct Workbench {
    database_path: Result<PathBuf, String>,
    state: ConnectionState,
    open_task: Option<Task<()>>,
}

enum ConnectionState {
    Disconnected,
    Opening,
    Connected {
        session: DemoSession,
        preview: NotesPreview,
    },
    Error(String),
}

impl Default for Workbench {
    fn default() -> Self {
        Self {
            database_path: demo_path().map_err(|error| error.to_string()),
            state: ConnectionState::Disconnected,
            open_task: None,
        }
    }
}

impl Workbench {
    /// Uses the real opening workflow with an explicit location, including isolated test files.
    pub fn with_database_path(path: PathBuf) -> Self {
        Self {
            database_path: Ok(path),
            state: ConnectionState::Disconnected,
            open_task: None,
        }
    }

    fn open_demo(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.state,
            ConnectionState::Opening | ConnectionState::Connected { .. }
        ) {
            return;
        }
        let path = match &self.database_path {
            Ok(path) => path.clone(),
            Err(error) => {
                self.state = ConnectionState::Error(error.clone());
                cx.notify();
                return;
            }
        };
        self.state = ConnectionState::Opening;
        cx.notify();
        let work = cx.background_spawn(async move {
            let session = DemoSession::open(path)?;
            let preview = session.preview()?;
            Ok::<_, crate::database::DatabaseError>((session, preview))
        });
        self.open_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.state = match result {
                    Ok((session, preview)) => ConnectionState::Connected { session, preview },
                    Err(error) => ConnectionState::Error(error.to_string()),
                };
                cx.notify();
            });
        }));
    }
}

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_open_database = cx.listener(|this, _, _, cx| {
            this.open_demo(cx);
        });
        let status: SharedString = match &self.state {
            ConnectionState::Disconnected => "No database connected".into(),
            ConnectionState::Opening => "Opening demo database…".into(),
            ConnectionState::Connected { .. } => "Demo database connected".into(),
            ConnectionState::Error(error) => {
                format!("Could not open demo database: {error}").into()
            }
        };
        let active = match &self.state {
            ConnectionState::Connected { session, preview } => Some((session.path(), preview)),
            _ => None,
        };
        let open_disabled = matches!(
            self.state,
            ConnectionState::Opening | ConnectionState::Connected { .. }
        );

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(header::render(cx, open_disabled, on_open_database))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(database_explorer::render(active.map(|(path, _)| path), cx))
                    .child(match active {
                        Some((_, preview)) => notes_panel::render(preview, cx).into_any_element(),
                        None => welcome_panel::render(cx).into_any_element(),
                    }),
            )
            .child(status_bar::render(status, cx))
    }
}
