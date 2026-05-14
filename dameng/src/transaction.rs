//! Transaction support — follows rust-postgres / sqlx patterns.
//!
//! A `Transaction` wraps a `Client` in a database transaction.
//! All operations on the `Transaction` are executed within the transaction.
//! On `Drop`, the transaction is automatically rolled back if not committed.

use crate::client::Client;
use crate::error::{Error, Result};
use crate::row::ResultSet;
use dameng_protocol::message::bind::BindParam;
use dameng_types::ToDmValue;

/// A database transaction, created by `Client::transaction()`.
///
/// All queries/executes on the `Transaction` are scoped to the transaction.
/// If not explicitly `commit()`ed, the transaction is rolled back on `Drop`.
///
/// # Example
///
/// ```ignore
/// let mut tx = client.transaction()?;
/// tx.execute("INSERT INTO t VALUES (1)")?;
/// tx.execute_with_params("INSERT INTO t VALUES (?)", &[&2])?;
/// tx.commit()?;
/// ```
pub struct Transaction {
    client: Option<Client>,
    /// True after commit() or rollback() is called.
    finished: bool,
}

impl Transaction {
    /// Commit the transaction, making all changes permanent.
    ///
    /// After commit, the transaction is finished and the underlying
    /// `Client` is returned for further use (outside the transaction).
    pub fn commit(mut self) -> Result<Client> {
        self.finish_with("COMMIT")
    }

    /// Roll back the transaction, discarding all changes.
    ///
    /// After rollback, the transaction is finished and the underlying
    /// `Client` is returned for further use (outside the transaction).
    pub fn rollback(mut self) -> Result<Client> {
        self.finish_with("ROLLBACK")
    }

    /// Execute a DML statement (INSERT/UPDATE/DELETE) within the transaction.
    ///
    /// Returns the number of rows affected.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        let client = self.client_mut()?;
        client.execute(sql)
    }

    /// Execute a DML statement with dynamic parameters within the transaction.
    ///
    /// Returns the number of rows affected.
    ///
    /// # SQLx-style usage
    ///
    /// ```ignore
    /// tx.execute_with_params("INSERT INTO t VALUES (?, ?)", &[&1, &"hello"])?;
    /// ```
    pub fn execute_with_params(&mut self, sql: &str, params: &[&dyn ToDmValue]) -> Result<u64> {
        let client = self.client_mut()?;
        client.execute_with_params(sql, params)
    }

    /// Execute a SELECT query within the transaction and return the result set.
    ///
    /// Does NOT auto-commit.
    pub fn query(&mut self, sql: &str) -> Result<ResultSet> {
        let client = self.client_mut()?;
        client.query(sql)
    }

    /// Execute a SELECT query with dynamic parameters within the transaction.
    ///
    /// Returns the result set.
    pub fn query_with_params(&mut self, sql: &str, params: &[&dyn ToDmValue]) -> Result<ResultSet> {
        let client = self.client_mut()?;
        client.query_with_params(sql, params)
    }

    // ── internal helpers ──

    fn client_mut(&mut self) -> Result<&mut Client> {
        if self.finished {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "transaction already finished",
            )));
        }
        self.client.as_mut().ok_or(Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "transaction client is gone",
        )))
    }

    fn finish_with(mut self, command: &str) -> Result<Client> {
        if self.finished {
            return self.take_client_with_error("transaction already finished");
        }
        self.finished = true;
        let mut client = self.client.take()
            .ok_or_else(|| Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other, "transaction client is gone"
            )))?;
        // Send COMMIT or ROLLBACK via execute (OPE(91) path)
        client.execute(command)?;
        // Restore auto_commit
        client.auto_commit = true;
        Ok(client)
    }

    fn take_client_with_error(&mut self, msg: &str) -> Result<Client> {
        self.client.take().ok_or_else(|| Error::Io(std::io::Error::new(
            std::io::ErrorKind::Other, msg,
        )))
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.finished {
            if let Some(ref mut client) = self.client {
                // Best-effort rollback on drop
                let _ = client.execute("ROLLBACK");
                client.auto_commit = true;
            }
        }
    }
}

/// Extension trait to add `transaction()` to `Client`.
///
/// This allows the user to write:
///
/// ```ignore
/// let tx = client.transaction()?;
/// tx.execute("INSERT ...")?;
/// tx.commit()?;
/// ```
impl Client {
    /// Begin a new transaction, returning a `Transaction` handle.
    ///
    /// This consumes the `Client` — all subsequent operations must go
    /// through the `Transaction`. After `commit()` or `rollback()`,
    /// the `Client` is returned.
    ///
    /// If the `Transaction` is dropped without calling `commit()` or
    /// `rollback()`, a `ROLLBACK` is sent automatically (best-effort).
    pub fn transaction(mut self) -> Result<Transaction> {
        self.begin()?;
        Ok(Transaction {
            client: Some(self),
            finished: false,
        })
    }
}
