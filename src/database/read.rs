use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use rusqlite::{
    Connection, ErrorCode,
    hooks::{AuthAction, AuthContext, Authorization},
    types::ValueRef,
};

use super::DemoSession;

// These bound input and Rust-owned results, not all SQLite VM allocations.
// Expansion-heavy blob/format functions are intentionally not in the allowlist.
pub(super) const MAX_SQL_BYTES: usize = 16 * 1024;
const MAX_COLUMNS: usize = 256;
const MAX_CELL_BYTES: usize = 1024 * 1024;
const MAX_RESULT_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_ROWS: usize = 200;
pub(super) const DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq)]
pub enum CellValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

#[derive(Debug, PartialEq)]
pub struct ReadResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<CellValue>>,
    pub truncated: bool,
}

/// Shared execution failures; the name is retained for existing read callers.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("Unsupported execution: {0}")]
    Unsupported(&'static str),
    #[error("SQLite execution failed: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("Database is locked")]
    Locked,
    #[error("Execution exceeded the five-second deadline")]
    Timeout,
}

impl DemoSession {
    /// Execute one SELECT (including WITH ... SELECT) on this session's connection.
    ///
    /// SQLite validates statement tails; an authorizer denies non-read actions
    /// and unaudited functions during preparation and any automatic reprepare.
    /// Return at most 200 rows, probing row 201 without copying it. Input is
    /// limited to 16 KiB, results to 256 columns, 1 MiB per text/blob/name and
    /// 8 MiB of copied payload. Limit violations are Unsupported, not truncation.
    /// Invalid UTF-8 text is a Sql error (never silently replaced).
    ///
    /// The five-second deadline includes preparation, stepping and copying;
    /// progress callbacks are cooperative, not a hard memory/OS sandbox.
    pub fn read(&mut self, sql: &str) -> Result<ReadResult, ReadError> {
        let deadline = Instant::now() + DEADLINE;
        if sql.len() > MAX_SQL_BYTES || sql.contains('\0') {
            return Err(ReadError::Unsupported("SQL exceeds 16 KiB or contains NUL"));
        }
        // Only an operation classifier: SQLite's parser and authorizer, not
        // this lexer, enforce read-only execution. VALUES and EXPLAIN are
        // readonly in SQLite, so readonly() alone cannot implement our scope.
        if !is_select(sql) {
            return Err(ReadError::Unsupported("Expected one SELECT statement"));
        }
        self.connection.busy_timeout(Duration::from_secs(1))?;
        let guard = ReadGuard::install(&self.connection, deadline)?;
        guard.check_deadline()?;
        let mut statement = guard.work(self.connection.prepare(sql))?;
        if !statement.readonly() || statement.is_explain() != 0 || statement.column_count() == 0 {
            return Err(ReadError::Unsupported("Expected one readonly SELECT"));
        }
        let mut bytes = 0;
        let columns = copy_columns(&statement, &guard, &mut bytes)?;
        guard.check_deadline()?;
        let mut cursor = guard.work(statement.query([]))?;
        let mut rows = Vec::new();
        let mut truncated = false;
        loop {
            guard.check_deadline()?;
            let Some(row) = guard.work(cursor.next())? else {
                break;
            };
            if rows.len() == MAX_ROWS {
                truncated = true;
                break;
            }
            rows.push(copy_row(row, columns.len(), &guard, &mut bytes)?);
        }
        guard.check_deadline()?;
        Ok(ReadResult {
            columns,
            rows,
            truncated,
        })
    }
}

pub(super) fn copy_columns(
    statement: &rusqlite::Statement<'_>,
    guard: &ReadGuard<'_>,
    bytes: &mut usize,
) -> Result<Vec<String>, ReadError> {
    if statement.column_count() > MAX_COLUMNS {
        return Err(ReadError::Unsupported("Result exceeds 256 columns"));
    }
    let mut columns = Vec::with_capacity(statement.column_count());
    for name in statement.column_names() {
        guard.check_deadline()?;
        reserve_payload(name.len(), bytes)?;
        columns.push(name.to_owned());
        guard.check_deadline()?;
    }
    Ok(columns)
}

pub(super) fn copy_row(
    row: &rusqlite::Row<'_>,
    column_count: usize,
    guard: &ReadGuard<'_>,
    bytes: &mut usize,
) -> Result<Vec<CellValue>, ReadError> {
    let mut cells = Vec::with_capacity(column_count);
    for column in 0..column_count {
        guard.check_deadline()?;
        let value = guard.work(row.get_ref(column))?;
        let size = match value {
            ValueRef::Null => 0,
            ValueRef::Integer(_) | ValueRef::Real(_) => 8,
            ValueRef::Text(value) | ValueRef::Blob(value) => value.len(),
        };
        reserve_payload(size, bytes)?;
        cells.push(match value {
            ValueRef::Null => CellValue::Null,
            ValueRef::Integer(value) => CellValue::Integer(value),
            ValueRef::Real(value) => CellValue::Real(value),
            ValueRef::Text(value) => CellValue::Text(
                std::str::from_utf8(value)
                    .map_err(|error| ReadError::Sql(rusqlite::Error::Utf8Error(error)))?
                    .to_owned(),
            ),
            ValueRef::Blob(value) => CellValue::Blob(value.to_vec()),
        });
        guard.check_deadline()?;
    }
    Ok(cells)
}

fn reserve_payload(size: usize, bytes: &mut usize) -> Result<(), ReadError> {
    if size > MAX_CELL_BYTES || size > MAX_RESULT_BYTES - *bytes {
        return Err(ReadError::Unsupported(
            "Cell exceeds 1 MiB or result exceeds 8 MiB",
        ));
    }
    *bytes += size;
    Ok(())
}

pub(super) struct ReadGuard<'a> {
    connection: &'a Connection,
    deadline: Instant,
    denied: Arc<AtomicBool>,
    internal_transaction: Arc<AtomicBool>,
}

impl<'a> ReadGuard<'a> {
    fn install(connection: &'a Connection, deadline: Instant) -> Result<Self, ReadError> {
        Self::install_with_mode(connection, deadline, false)
    }

    pub(super) fn install_write(
        connection: &'a Connection,
        deadline: Instant,
    ) -> Result<Self, ReadError> {
        Self::install_with_mode(connection, deadline, true)
    }

    fn install_with_mode(
        connection: &'a Connection,
        deadline: Instant,
        writes: bool,
    ) -> Result<Self, ReadError> {
        // Construct first so even a partially installed hook is cleaned up.
        let guard = Self {
            connection,
            deadline,
            denied: Arc::new(AtomicBool::new(false)),
            internal_transaction: Arc::new(AtomicBool::new(false)),
        };
        let denied = Arc::clone(&guard.denied);
        let internal_transaction = Arc::clone(&guard.internal_transaction);
        connection.authorizer(Some(move |context: AuthContext<'_>| {
            let allowed = match context.action {
                AuthAction::Select | AuthAction::Recursive => true,
                AuthAction::Read {
                    table_name,
                    column_name,
                } => {
                    // SQLite emits an unspecified database for unqualified
                    // table-only reads (COUNT(*)) and CTE/subquery references.
                    (context.database_name == Some("main")
                        || (context.database_name.is_none() && column_name.is_empty()))
                        && !table_name.to_ascii_lowercase().starts_with("pragma_")
                }
                AuthAction::Function { function_name } => safe_function(function_name),
                AuthAction::Insert { table_name }
                | AuthAction::Update { table_name, .. }
                | AuthAction::Delete { table_name } => {
                    writes
                        && context.database_name == Some("main")
                        && table_name.eq_ignore_ascii_case("notes")
                }
                AuthAction::Transaction { .. } => {
                    writes && internal_transaction.load(Ordering::Relaxed)
                }
                _ => false,
            };
            if allowed {
                Authorization::Allow
            } else {
                denied.store(true, Ordering::Relaxed);
                Authorization::Deny
            }
        }))?;
        connection.progress_handler(1000, Some(move || Instant::now() >= deadline))?;
        guard.check_deadline()?;
        Ok(guard)
    }

    pub(super) fn check_deadline(&self) -> Result<(), ReadError> {
        if Instant::now() >= self.deadline {
            Err(ReadError::Timeout)
        } else {
            Ok(())
        }
    }

    pub(super) fn work<T>(&self, result: rusqlite::Result<T>) -> Result<T, ReadError> {
        self.check_deadline()?;
        result.map_err(|error| {
            if self.denied.load(Ordering::Relaxed) {
                ReadError::Unsupported("SQLite action or function is not allowed")
            } else {
                match error.sqlite_error_code() {
                    Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => ReadError::Locked,
                    Some(ErrorCode::OperationInterrupted) => ReadError::Timeout,
                    _ if matches!(error, rusqlite::Error::MultipleStatement) => {
                        ReadError::Unsupported("Multiple statements are not allowed")
                    }
                    _ => ReadError::Sql(error),
                }
            }
        })
    }

    // Enable only transaction authorization, only while invoking our own SQL.
    // The reset guard also runs on unwind. User preparation/reprepare never
    // happens in this phase, including preparation of statement tails.
    pub(super) fn internal<T>(&self, action: impl FnOnce() -> T) -> T {
        struct Reset<'a>(&'a AtomicBool);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Relaxed);
            }
        }
        self.internal_transaction.store(true, Ordering::Relaxed);
        let _reset = Reset(&self.internal_transaction);
        action()
    }

    pub(super) fn clear_progress(&self) {
        let _ = self.connection.progress_handler(0, None::<fn() -> bool>);
    }
}

impl Drop for ReadGuard<'_> {
    fn drop(&mut self) {
        // DemoSession privately owns a fresh connection with no other hooks.
        // None restores that baseline on every return/unwind; these APIs can
        // fail only for an unowned connection, which this session never uses.
        self.clear_progress();
        let _ = self
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>);
    }
}

fn safe_function(name: &str) -> bool {
    // Closed list of SQLite's side-effect-free core/aggregate/window built-ins.
    // No extension/file functions, pragma functions, randomblob/zeroblob,
    // printf/format, or unknown future functions are authorized.
    matches!(
        name.to_ascii_lowercase().as_str(),
        "abs"
            | "avg"
            | "char"
            | "coalesce"
            | "concat"
            | "concat_ws"
            | "count"
            | "date"
            | "datetime"
            | "glob"
            | "group_concat"
            | "hex"
            | "if"
            | "ifnull"
            | "iif"
            | "instr"
            | "julianday"
            | "length"
            | "like"
            | "likelihood"
            | "likely"
            | "lower"
            | "ltrim"
            | "max"
            | "min"
            | "nullif"
            | "octet_length"
            | "quote"
            | "random"
            | "replace"
            | "round"
            | "rtrim"
            | "sign"
            | "sqlite_source_id"
            | "sqlite_version"
            | "strftime"
            | "string_agg"
            | "substr"
            | "substring"
            | "sum"
            | "time"
            | "timediff"
            | "total"
            | "trim"
            | "typeof"
            | "unhex"
            | "unicode"
            | "unixepoch"
            | "unlikely"
            | "upper"
            | "row_number"
            | "rank"
            | "dense_rank"
            | "percent_rank"
            | "cume_dist"
            | "ntile"
            | "lag"
            | "lead"
            | "first_value"
            | "last_value"
            | "nth_value"
    )
}

fn is_select(sql: &str) -> bool {
    main_operation(sql) == Some(Operation::Select)
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Operation {
    Select,
    Insert,
    Update,
    Delete,
}

impl Operation {
    fn from_token(token: Option<SqlToken<'_>>) -> Option<Self> {
        let token = token?;
        if token.keyword(b"select") {
            Some(Self::Select)
        } else if token.keyword(b"insert") {
            Some(Self::Insert)
        } else if token.keyword(b"update") {
            Some(Self::Update)
        } else if token.keyword(b"delete") {
            Some(Self::Delete)
        } else {
            None
        }
    }
}

pub(super) fn main_operation(sql: &str) -> Option<Operation> {
    let mut tokens = SqlTokens(sql.as_bytes());
    let mut token = tokens.next();
    while token == Some(SqlToken::Symbol(b';')) {
        token = tokens.next();
    }
    if !token.is_some_and(|token| token.keyword(b"with")) {
        return Operation::from_token(token);
    }
    token = tokens.next();
    if token.is_some_and(|token| token.keyword(b"recursive")) {
        token = tokens.next();
    }
    loop {
        // A CTE name is an identifier, never a candidate main operation.
        // SQLite still decides which unquoted/quoted names are valid.
        if !matches!(token, Some(SqlToken::Word(_) | SqlToken::Quoted)) {
            return None;
        }
        token = tokens.next();
        if token == Some(SqlToken::Symbol(b'(')) {
            if !skip_parentheses(&mut tokens) {
                return None;
            }
            token = tokens.next();
        }
        if !token.is_some_and(|token| token.keyword(b"as")) {
            return None;
        }
        token = tokens.next();
        if token.is_some_and(|token| token.keyword(b"not")) {
            if !tokens
                .next()
                .is_some_and(|token| token.keyword(b"materialized"))
            {
                return None;
            }
            token = tokens.next();
        } else if token.is_some_and(|token| token.keyword(b"materialized")) {
            token = tokens.next();
        }
        if token != Some(SqlToken::Symbol(b'(')) || !skip_parentheses(&mut tokens) {
            return None;
        }
        token = tokens.next();
        if token != Some(SqlToken::Symbol(b',')) {
            return Operation::from_token(token);
        }
        token = tokens.next();
    }
}

// The opening parenthesis has already been consumed. Quoted text and comments
// are opaque tokens, so their parentheses cannot change nesting depth.
fn skip_parentheses(tokens: &mut SqlTokens<'_>) -> bool {
    let mut depth = 1usize;
    for token in tokens {
        match token {
            SqlToken::Symbol(b'(') => depth += 1,
            SqlToken::Symbol(b')') => {
                depth -= 1;
                if depth == 0 {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[derive(Clone, Copy, PartialEq)]
enum SqlToken<'a> {
    Word(&'a [u8]),
    Quoted,
    Symbol(u8),
}

impl SqlToken<'_> {
    fn keyword(self, keyword: &[u8]) -> bool {
        matches!(self, Self::Word(word) if word.eq_ignore_ascii_case(keyword))
    }
}

struct SqlTokens<'a>(&'a [u8]);

impl<'a> Iterator for SqlTokens<'a> {
    type Item = SqlToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let &byte = self.0.first()?;
            if byte.is_ascii_whitespace() {
                self.0 = &self.0[1..];
            } else if self.0.starts_with(b"\xef\xbb\xbf") {
                self.0 = &self.0[3..];
            } else if self.0.starts_with(b"--") {
                let end = self
                    .0
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .unwrap_or(self.0.len());
                self.0 = &self.0[end..];
            } else if self.0.starts_with(b"/*") {
                let end = self.0[2..].windows(2).position(|pair| pair == b"*/")? + 4;
                self.0 = &self.0[end..];
            } else {
                break;
            }
        }
        let input = self.0;
        let byte = input[0];
        if matches!(byte, b'\'' | b'"' | b'`' | b'[') {
            let end = if byte == b'[' { b']' } else { byte };
            let mut i = 1;
            while i < input.len() {
                if input[i] == end {
                    i += 1;
                    if byte != b'[' && input.get(i) == Some(&end) {
                        i += 1;
                    } else {
                        self.0 = &input[i..];
                        return Some(SqlToken::Quoted);
                    }
                } else {
                    i += 1;
                }
            }
            return None;
        }
        // SQLite identifiers include high-bit bytes and digits (and '$' after
        // the first byte). Group digit-led words too; SQLite validates them,
        // rather than accidentally treating an embedded suffix as a keyword.
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 128 {
            let end = input
                .iter()
                .position(|&byte| {
                    !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || byte >= 128)
                })
                .unwrap_or(input.len());
            self.0 = &input[end..];
            Some(SqlToken::Word(&input[..end]))
        } else {
            self.0 = &input[1..];
            Some(SqlToken::Symbol(byte))
        }
    }
}
