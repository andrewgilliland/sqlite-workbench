use sqlite_workbench::database::{DemoSession, Note};

#[test]
fn explicit_demo_directory_is_absolute_and_does_not_touch_default_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = sqlite_workbench::database::demo_path_in(directory.path()).unwrap();
    assert_eq!(path, directory.path().join("demo.sqlite3"));
    assert_ne!(path, sqlite_workbench::database::demo_path().unwrap());
    assert!(!path.exists());
    drop(DemoSession::open(&path).unwrap());
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .preview()
            .unwrap()
            .notes
            .len(),
        2
    );
    assert!(
        sqlite_workbench::database::demo_path_in(std::path::Path::new("relative-dir")).is_err()
    );
}

#[test]
fn rejects_non_rowid_primary_keys_and_hidden_columns() {
    let directory = tempfile::tempdir().unwrap();
    for (index, schema) in [
        "CREATE TABLE notes (id INTEGER PRIMARY KEY DESC, body TEXT)",
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT, extra TEXT GENERATED ALWAYS AS (body) VIRTUAL)",
    ].iter().enumerate() {
        let path = directory.path().join(format!("incompatible-{index}.sqlite3"));
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(schema).unwrap();
        drop(connection);
        let before = std::fs::read(&path).unwrap();
        assert!(DemoSession::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[cfg(unix)]
#[test]
fn initialization_io_failure_leaves_no_published_or_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "initialization_io_failure_child", "--nocapture"])
        .env("SQLITE_WORKBENCH_IO_FAILURE_PATH", &path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .preview()
            .unwrap()
            .notes
            .len(),
        2
    );
}

#[cfg(unix)]
#[test]
fn initialization_io_failure_child() {
    let Some(path) = std::env::var_os("SQLITE_WORKBENCH_IO_FAILURE_PATH") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    // Restrict only this subprocess: force SQLite to fail mid-initialization instead
    // of killing the process, so normal transaction and temporary-file cleanup runs.
    unsafe {
        let mut previous: libc::rlimit = std::mem::zeroed();
        assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE, &mut previous), 0);
        let limited = libc::rlimit {
            rlim_cur: 4096,
            rlim_max: previous.rlim_max,
        };
        libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &limited), 0);
        let result = DemoSession::open(&path);
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE, &previous), 0);
        assert!(result.is_err());
    }
    assert!(!path.exists());
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        0
    );
}

#[test]
fn reopening_preserves_external_updates_and_empty_tables() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    drop(DemoSession::open(&path).unwrap());
    let external = rusqlite::Connection::open(&path).unwrap();
    external
        .execute(
            "UPDATE notes SET body = 'Externally changed' WHERE id = 1",
            [],
        )
        .unwrap();
    external
        .execute("INSERT INTO notes (id, body) VALUES (3, NULL)", [])
        .unwrap();
    let reopened = DemoSession::open(&path).unwrap();
    assert_eq!(
        reopened.preview().unwrap().notes[0].body.as_deref(),
        Some("Externally changed")
    );
    assert_eq!(reopened.preview().unwrap().notes[2].body, None);
    drop(reopened);
    external.execute("DELETE FROM notes", []).unwrap();
    drop(external);
    let empty = DemoSession::open(&path).unwrap();
    assert!(empty.preview().unwrap().notes.is_empty());
}

#[test]
fn existing_invalid_and_incompatible_files_are_not_modified() {
    let directory = tempfile::tempdir().unwrap();
    let invalid = directory.path().join("invalid.sqlite3");
    std::fs::write(&invalid, b"not a database").unwrap();
    assert!(DemoSession::open(&invalid).is_err());
    assert_eq!(std::fs::read(&invalid).unwrap(), b"not a database");
    let incompatible = directory.path().join("incompatible.sqlite3");
    let connection = rusqlite::Connection::open(&incompatible).unwrap();
    connection
        .execute_batch("CREATE TABLE notes (body TEXT); INSERT INTO notes VALUES ('keep me');")
        .unwrap();
    drop(connection);
    let before = std::fs::read(&incompatible).unwrap();
    assert!(DemoSession::open(&incompatible).is_err());
    assert_eq!(std::fs::read(&incompatible).unwrap(), before);
    let empty = directory.path().join("empty.sqlite3");
    std::fs::write(&empty, []).unwrap();
    assert!(DemoSession::open(&empty).is_err());
    assert_eq!(std::fs::metadata(&empty).unwrap().len(), 0);
}

#[test]
fn failed_creation_does_not_publish_an_empty_database() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("not-a-directory");
    std::fs::write(&parent, b"preserve").unwrap();
    assert!(DemoSession::open(parent.join("demo.sqlite3")).is_err());
    assert_eq!(std::fs::read(parent).unwrap(), b"preserve");
}

#[cfg(unix)]
#[test]
fn read_only_files_and_unwritable_directories_fail_without_replacement() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    drop(DemoSession::open(&path).unwrap());
    let before = std::fs::read(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400)).unwrap();
    assert!(DemoSession::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = DemoSession::open(directory.path().join("new.sqlite3"));
    std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(!directory.path().join("new.sqlite3").exists());
}

#[test]
fn concurrent_creation_publishes_one_seeded_database() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    std::thread::scope(|scope| {
        let first = scope.spawn(|| DemoSession::open(&path).unwrap().preview().unwrap().notes);
        let second = scope.spawn(|| DemoSession::open(&path).unwrap().preview().unwrap().notes);
        assert_eq!(first.join().unwrap(), second.join().unwrap());
    });
    assert_eq!(
        DemoSession::open(&path)
            .unwrap()
            .preview()
            .unwrap()
            .notes
            .len(),
        2
    );
}

#[test]
fn preview_is_bounded_and_reports_more_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("demo.sqlite3");
    let session = DemoSession::open(&path).unwrap();
    let external = rusqlite::Connection::open(&path).unwrap();
    external.execute_batch("DELETE FROM notes; WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x < 200) INSERT INTO notes SELECT x, 'preview' FROM n;").unwrap();
    assert_eq!(session.preview().unwrap().notes.len(), 200);
    assert!(!session.preview().unwrap().truncated);
    external
        .execute("INSERT INTO notes VALUES (201, 'extra')", [])
        .unwrap();
    assert_eq!(session.preview().unwrap().notes.len(), 200);
    assert!(session.preview().unwrap().truncated);
}

#[test]
fn default_path_is_absolute_and_uses_platform_app_data() {
    let path = sqlite_workbench::database::demo_path().unwrap();
    assert!(path.is_absolute());
    assert_eq!(
        path,
        dirs::data_local_dir()
            .unwrap()
            .join("SQLite Workbench/demo.sqlite3")
    );
}

#[test]
fn creates_a_seeded_file_and_reopens_without_reseeding() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested/demo.sqlite3");
    let session = DemoSession::open(&path).unwrap();
    assert_eq!(session.path(), path);
    assert_eq!(
        session.preview().unwrap().notes,
        vec![
            Note {
                id: 1,
                body: Some("Welcome to SQLite Workbench".into())
            },
            Note {
                id: 2,
                body: Some("Your changes stay in this local file".into())
            },
        ]
    );
    drop(session);
    let reopened = DemoSession::open(&path).unwrap();
    assert_eq!(reopened.preview().unwrap().notes.len(), 2);
}
