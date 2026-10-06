use std::time::{Duration, Instant};

use rusqlite::Transaction;

use super::{
    DemoSession, ReadError, ReadResult,
    read::{DEADLINE, MAX_SQL_BYTES, Operation, ReadGuard, main_operation},
};

#[derive(Debug, PartialEq)]
pub enum ExecutionResult {
    Read(ReadResult),
    Write { affected_rows: usize },
}

impl DemoSession {
    /// Execute one SELECT, INSERT, UPDATE or DELETE, including WITH forms.
    ///
    /// SELECT uses the existing bounded, read-only result contract. Writes
    /// target main.notes only, disallow RETURNING, and report SQLite's affected
    /// row count (excluding trigger effects) only after an immediate commit.
    /// SQLite validates all statement tails before any changes. Schema changes,
    /// transaction commands and unaudited functions are not supported.
    ///
    /// Each write owns a transaction; every failure rolls it back before return.
    /// A one-second lock wait and cooperative five-second deadline cover
    /// preparation, stepping and commit, not a hard memory or OS bound.
    pub fn execute(&mut self, sql: &str) -> Result<ExecutionResult, ReadError> {
        if sql.len() > MAX_SQL_BYTES || sql.contains('\0') {
            return Err(ReadError::Unsupported("SQL exceeds 16 KiB or contains NUL"));
        }
        let Some(operation) = main_operation(sql) else {
            return Err(ReadError::Unsupported(
                "Expected one SELECT, INSERT, UPDATE or DELETE",
            ));
        };
        if operation == Operation::Select {
            return self.read(sql).map(ExecutionResult::Read);
        }
        let deadline = Instant::now() + DEADLINE;
        self.connection.busy_timeout(Duration::from_secs(1))?;
        let guard = ReadGuard::install_write(&self.connection, deadline)?;
        // rusqlite recursively prepares the full tail with SQLite's parser.
        // No user statement steps, or internal transaction starts, until that
        // validation and the no-RETURNING check have both succeeded.
        let mut statement = guard.work(self.connection.prepare(sql))?;
        if statement.readonly() || statement.is_explain() != 0 || statement.column_count() != 0 {
            return Err(ReadError::Unsupported("Expected a write without RETURNING"));
        }
        guard.check_deadline()?;
        let transaction = match guard.internal(|| self.connection.unchecked_transaction()) {
            Ok(transaction) => transaction,
            Err(error) => return guard.work(Err(error)),
        };
        let pending = PendingWrite {
            transaction: Some(transaction),
            guard: &guard,
        };
        // Construct the rollback owner before checking elapsed time, so even a
        // successful BEGIN that crossed the deadline cannot leak a transaction.
        let result = guard
            .check_deadline()
            .and_then(|()| guard.work(statement.execute([])));
        drop(statement);
        let affected_rows = result?;
        pending.commit()?;
        Ok(ExecutionResult::Write { affected_rows })
    }
}

struct PendingWrite<'a, 'guard> {
    transaction: Option<Transaction<'a>>,
    guard: &'guard ReadGuard<'a>,
}

impl PendingWrite<'_, '_> {
    fn commit(self) -> Result<(), ReadError> {
        self.guard.check_deadline()?;
        // Do not consume Transaction::commit(): on failure it would attempt
        // rollback before we could clear an expired progress callback.
        let result = self
            .guard
            .internal(|| self.transaction.as_ref().unwrap().execute_batch("COMMIT"));
        match result {
            // Successful COMMIT is the point of no return. Never report a
            // post-commit timeout for data that can no longer be rolled back.
            // The deadline was checked immediately before it, and the progress
            // callback remains installed throughout this cooperative operation.
            Ok(()) => Ok(()),
            Err(error) => self.guard.work(Err(error)),
        }
    }
}

impl Drop for PendingWrite<'_, '_> {
    fn drop(&mut self) {
        // Recovery must not be interrupted by the expired execution deadline.
        // Transaction::drop rolls back only if SQLite is still in a transaction
        // (OR ROLLBACK and interruption may already have rolled it back).
        self.guard.clear_progress();
        self.guard.internal(|| drop(self.transaction.take()));
    }
}
