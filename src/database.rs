use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{Connection, OpenFlags};

#[path = "database/read.rs"]
mod read;
pub use read::{CellValue, ReadError, ReadResult};

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("File access failed: {0}")]
    File(#[from] io::Error),
    #[error("SQLite open or preview failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Existing database has an incompatible notes schema; it was not changed")]
    IncompatibleSchema,
    #[error("The platform app-data directory is unavailable")]
    MissingDataDirectory,
}

/// Independent of the current working directory; callers can supply isolated paths in tests.
pub fn demo_path() -> Result<PathBuf, DatabaseError> {
    Ok(dirs::data_local_dir()
        .ok_or(DatabaseError::MissingDataDirectory)?
        .join("SQLite Workbench")
        .join("demo.sqlite3"))
}

#[derive(Debug, PartialEq, Eq)]
pub struct Note {
    pub id: i64,
    pub body: Option<String>,
}

pub struct DemoSession {
    connection: Connection,
    path: PathBuf,
}

pub struct NotesPreview {
    pub notes: Vec<Note>,
    pub truncated: bool,
}

impl DemoSession {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = std::path::absolute(path)?;
        match fs::symlink_metadata(&path) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::create(&path)?,
            Err(error) => return Err(error.into()),
        }
        // Never let SQLite create an empty replacement for a missing existing file.
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        if connection.is_readonly("main")? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Demo database is read-only",
            )
            .into());
        }
        connection.busy_timeout(Duration::from_secs(1))?;
        let columns = {
            let mut statement = connection.prepare("PRAGMA table_xinfo(notes)")?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };
        // A rowid-alias INTEGER PRIMARY KEY has no separate primary-key index.
        // DESC and WITHOUT ROWID variants do, despite identical table_info types.
        let has_separate_primary_key: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_index_list('notes') WHERE origin = 'pk')",
            [],
            |row| row.get(0),
        )?;
        if columns.len() != 2
            || has_separate_primary_key
            || columns[0].0 != "id"
            || !columns[0].1.eq_ignore_ascii_case("INTEGER")
            || columns[0].2 != 1
            || columns[1].0 != "body"
            || !columns[1].1.eq_ignore_ascii_case("TEXT")
            || columns[1].2 != 0
        {
            return Err(DatabaseError::IncompatibleSchema);
        }
        Ok(Self { connection, path })
    }

    fn create(path: &Path) -> Result<(), DatabaseError> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Database path has no parent"))?;
        fs::create_dir_all(parent)?;
        let temporary = tempfile::NamedTempFile::new_in(parent)?;
        {
            let mut connection = Connection::open(temporary.path())?;
            let transaction = connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT);
                 INSERT INTO notes (id, body) VALUES
                 (1, 'Welcome to SQLite Workbench'),
                 (2, 'Your changes stay in this local file');",
            )?;
            transaction.commit()?;
            connection.close().map_err(|(_, error)| error)?;
        }
        temporary.as_file().sync_all()?;
        // Publish only a fully initialized file, without replacing a concurrently created one.
        match temporary.persist_noclobber(path) {
            Ok(_) => Ok(()),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error.error.into()),
        }
    }

    pub fn preview(&self) -> Result<NotesPreview, DatabaseError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, body FROM notes ORDER BY id LIMIT 201")?;
        let mut notes = statement
            .query_map([], |row| {
                Ok(Note {
                    id: row.get(0)?,
                    body: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let truncated = notes.len() > 200;
        notes.truncate(200);
        Ok(NotesPreview { notes, truncated })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
