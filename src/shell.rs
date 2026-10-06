#[path = "shell/database_explorer.rs"]
mod database_explorer;
#[path = "shell/header.rs"]
mod header;
#[path = "shell/notes_panel.rs"]
mod notes_panel;
#[path = "shell/query_panel.rs"]
mod query_panel;
#[path = "shell/query_results.rs"]
mod query_results;
#[path = "shell/status_bar.rs"]
mod status_bar;
#[path = "shell/welcome_panel.rs"]
mod welcome_panel;

use crate::database::{DemoSession, ExecutionResult, NotesPreview, ReadError, demo_path};
use gpui_kit::component::{ActiveTheme, input::EditorState};
use gpui_kit::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub struct Workbench {
    database_path: Result<PathBuf, String>,
    state: ConnectionState,
    open_task: Option<Task<()>>,
    editor: Option<Entity<EditorState>>,
    running: bool,
    query_task: Option<Task<()>>,
    query_started: bool,
    results: Option<ExecutionResult>,
    query_status: Option<SharedString>,
}

enum ConnectionState {
    Disconnected,
    Opening,
    Connected {
        session: Arc<Mutex<DemoSession>>,
        path: PathBuf,
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
            editor: None,
            running: false,
            query_task: None,
            query_started: false,
            results: None,
            query_status: None,
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
            editor: None,
            running: false,
            query_task: None,
            query_started: false,
            results: None,
            query_status: None,
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
            let path = session.path().to_path_buf();
            Ok::<_, crate::database::DatabaseError>((Arc::new(Mutex::new(session)), path, preview))
        });
        self.open_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            let _ = this.update(cx, |this, cx| {
                this.state = match result {
                    Ok((session, path, preview)) => ConnectionState::Connected {
                        session,
                        path,
                        preview,
                    },
                    Err(error) => ConnectionState::Error(error.to_string()),
                };
                cx.notify();
            });
        }));
    }

    fn run_query(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        let ConnectionState::Connected { session, .. } = &self.state else {
            return;
        };
        let Some(editor) = &self.editor else {
            return;
        };
        let sql = editor.read(cx).value().to_string();
        let session = Arc::clone(session);
        self.running = true;
        self.query_started = true;
        self.results = None;
        self.query_status = Some("Running query…".into());
        cx.notify();
        // The UI only clones the handle. All session locking and SQLite work
        // stays here, including lock waits and the database execution deadline.
        let work = cx.background_spawn(async move {
            session
                .lock()
                .expect("database worker panicked")
                .execute(&sql)
        });
        self.query_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            // A released workbench cannot receive a late completion.
            let _ = this.update(cx, |this, cx| {
                this.running = false;
                match result {
                    Ok(result) => {
                        this.query_status = Some(match &result {
                            ExecutionResult::Read(result) => format!(
                                "Query complete: {} rows{}",
                                result.rows.len(),
                                if result.truncated {
                                    " (truncated to 200 rows)"
                                } else {
                                    ""
                                }
                            )
                            .into(),
                            ExecutionResult::Write { affected_rows } => {
                                format!("Write committed: {affected_rows} rows affected").into()
                            }
                        });
                        this.results = Some(result);
                    }
                    Err(error) => {
                        let category = match &error {
                            ReadError::Unsupported(_) => "Unsupported SQL",
                            ReadError::Sql(_) => "Query error",
                            ReadError::Locked => "Database locked",
                            ReadError::Timeout => "Query timed out",
                        };
                        this.query_status = Some(match error {
                            ReadError::Locked | ReadError::Timeout => category.into(),
                            _ => format!("{category}: {error}").into(),
                        });
                        this.results = None;
                    }
                }
                cx.notify();
            });
        }));
    }
}

impl Render for Workbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self
            .editor
            .get_or_insert_with(|| {
                cx.new(|cx| {
                    EditorState::new(window, cx)
                        .language("sql")
                        .default_value("SELECT id, body FROM notes ORDER BY id;")
                })
            })
            .clone();
        let on_open_database = cx.listener(|this, _, _, cx| {
            this.open_demo(cx);
        });
        let on_run = cx.listener(|this, _, _, cx| this.run_query(cx));
        let status: SharedString = match &self.state {
            ConnectionState::Disconnected => "No database connected".into(),
            ConnectionState::Opening => "Opening demo database…".into(),
            ConnectionState::Connected { .. } => self
                .query_status
                .clone()
                .unwrap_or_else(|| "Demo database connected".into()),
            ConnectionState::Error(error) => {
                format!("Could not open demo database: {error}").into()
            }
        };
        let active = match &self.state {
            ConnectionState::Connected { path, preview, .. } => Some((path.as_path(), preview)),
            _ => None,
        };
        let open_disabled = matches!(
            self.state,
            ConnectionState::Opening | ConnectionState::Connected { .. }
        );
        let (read_result, write_summary) = match &self.results {
            Some(ExecutionResult::Read(result)) => (Some(result), None),
            Some(ExecutionResult::Write { affected_rows }) => (
                None,
                Some(format!("Write committed: {affected_rows} rows affected")),
            ),
            None => (None, None),
        };

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
                    .child(query_panel::render(
                        &editor,
                        active.is_none() || self.running,
                        on_run,
                        if self.query_started {
                            query_results::render(read_result, write_summary.as_deref(), cx)
                                .into_any_element()
                        } else {
                            match active {
                                Some((_, preview)) => {
                                    notes_panel::render(preview, cx).into_any_element()
                                }
                                None => welcome_panel::render(cx).into_any_element(),
                            }
                        },
                        cx,
                    )),
            )
            .child(status_bar::render(status, cx))
    }
}
