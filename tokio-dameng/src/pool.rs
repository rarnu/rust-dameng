//! Async connection pool for tokio-dameng.
//!
//! Provides a simple, tokio-native connection pool without external pooling
//! dependencies. Uses `std::sync::Mutex<Vec<Client>>` for idle pool management
//! (since Drop is synchronous) with a `Semaphore` to limit concurrent connections.
//!
//! # Example
//!
//! ```no_run
//! use tokio_dameng::pool::{Pool, PoolConfig};
//!
//! # async fn run() {
//! let pool = Pool::new("127.0.0.1", 5236, "SYSDBA", "SYSDBA", PoolConfig::default());
//! let mut conn = pool.get().await.unwrap();
//! let rs = conn.query("SELECT 1 FROM DUAL").await.unwrap();
//! drop(conn); // Returns connection to pool
//! # }
//! ```

use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::Semaphore;
use tokio::time::{timeout, Duration};

use crate::client::Client;
use crate::error::{Error, Result};

/// A checked-out connection from the pool.
///
/// Automatically returns the connection to the idle pool when dropped.
/// The semaphore permit was forgotten (not auto-released) so we add it back in Drop.
pub struct PooledConnection {
    conn: Option<Client>,
    semaphore: Arc<Semaphore>,
    return_to_pool: Option<Arc<StdMutex<Vec<Client>>>>,
}

impl PooledConnection {
    /// Delegate query execution.
    pub async fn query(&mut self, sql: &str) -> Result<crate::row::ResultSet> {
        self.conn.as_mut().unwrap().query(sql).await
    }

    /// Delegate execute.
    pub async fn execute(&mut self, sql: &str) -> Result<u64> {
        self.conn.as_mut().unwrap().execute(sql).await
    }

    /// Delegate commit.
    pub async fn commit(&mut self) -> Result<()> {
        self.conn.as_mut().unwrap().commit().await
    }

    /// Delegate rollback.
    pub async fn rollback(&mut self) -> Result<()> {
        self.conn.as_mut().unwrap().rollback().await
    }

    /// Delegate begin.
    pub async fn begin(&mut self) -> Result<()> {
        self.conn.as_mut().unwrap().begin().await
    }

    /// Check if connection is still valid.
    pub async fn ping(&mut self) -> bool {
        self.conn.as_mut().unwrap().query("SELECT 1 FROM DUAL").await.is_ok()
    }

    /// Execute DML with dynamic parameters.
    pub async fn execute_with_params(
        &mut self,
        sql: &str,
        params: &[&dyn dameng_types::ToDmValue],
    ) -> Result<u64> {
        self.conn.as_mut().unwrap().execute_with_params(sql, params).await
    }

    /// Execute SELECT with dynamic parameters.
    pub async fn query_with_params(
        &mut self,
        sql: &str,
        params: &[&dyn dameng_types::ToDmValue],
    ) -> Result<crate::row::ResultSet> {
        self.conn.as_mut().unwrap().query_with_params(sql, params).await
    }

    /// Fetch more rows from a result set.
    pub async fn fetch_more(
        &mut self,
        result_set: &mut crate::row::ResultSet,
        start_row: usize,
        prefetch_bytes: i32,
    ) -> Result<u64> {
        self.conn.as_mut().unwrap().fetch_more(result_set, start_row, prefetch_bytes).await
    }

    /// Set transaction isolation level.
    pub async fn set_isolation(
        &mut self,
        level: dameng_protocol::message::isolation::IsolationLevel,
    ) -> Result<()> {
        self.conn.as_mut().unwrap().set_isolation(level).await
    }

    /// Read LOB data from a LOB locator.
    pub async fn read_lob(&mut self, locator: &dameng_types::LobLocator) -> Result<Vec<u8>> {
        self.conn.as_mut().unwrap().read_lob(locator).await
    }

    /// Read output parameters from a stored procedure call.
    pub fn read_output_params(
        &self,
        params: &[dameng_protocol::message::BindParam],
    ) -> Vec<(i32, Vec<u8>)> {
        self.conn.as_ref().unwrap().read_output_params(params)
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // Return connection to idle pool (using std::sync::Mutex for sync Drop)
        if let Some(conn) = self.conn.take() {
            if let Some(return_to_pool) = self.return_to_pool.take() {
                let mut idle = return_to_pool.lock().unwrap_or_else(|e| e.into_inner());
                idle.push(conn);
            }
        }
        // Release the semaphore permit (we forgot the Permit in get(), so it wasn't auto-released)
        self.semaphore.add_permits(1);
    }
}

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of idle connections to maintain.
    pub min_idle: usize,
    /// Maximum number of connections in the pool.
    pub max_size: usize,
    /// Maximum lifetime of a connection before it's recycled (None = unlimited).
    pub max_lifetime: Option<Duration>,
    /// Timeout when waiting for a connection from the pool.
    pub wait_timeout: Duration,
    /// How often to check for idle connections to maintain min_idle.
    pub idle_check_interval: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_idle: 1,
            max_size: 10,
            max_lifetime: Some(Duration::from_secs(300)),
            wait_timeout: Duration::from_secs(30),
            idle_check_interval: Duration::from_secs(30),
        }
    }
}

/// Async connection pool for Dameng database.
///
/// Thread-safe and cloneable. Creates connections lazily up to `max_size`.
#[derive(Clone)]
pub struct Pool {
    config: PoolConfig,
    host: String,
    port: u16,
    user: String,
    pass: String,
    semaphore: Arc<Semaphore>,
    idle: Arc<StdMutex<Vec<Client>>>,
}

impl Pool {
    /// Create a new connection pool.
    pub fn new(host: &str, port: u16, user: &str, pass: &str, config: PoolConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_size));
        let idle = Arc::new(StdMutex::new(Vec::new()));

        Self {
            config,
            host: host.to_string(),
            port,
            user: user.to_string(),
            pass: pass.to_string(),
            semaphore,
            idle,
        }
    }

    /// Get a connection from the pool.
    ///
    /// Waits up to `wait_timeout` for an available connection.
    /// Creates new connections up to `max_size` if needed.
    pub async fn get(&self) -> Result<PooledConnection> {
        // Acquire a semaphore permit (blocks if pool is full)
        let permit = match timeout(self.config.wait_timeout, self.semaphore.acquire()).await {
            Ok(Ok(p)) => p,
            Ok(Err(_)) => {
                return Err(Error::ConnectionFailed("Semaphore acquire failed".to_string()));
            }
            Err(_) => {
                return Err(Error::ConnectionFailed("Pool acquire timeout".to_string()));
            }
        };

        // Forget the permit so it doesn't auto-release when dropped at end of this function.
        // We will manually add_permits(1) in PooledConnection::Drop instead.
        std::mem::forget(permit);

        // Try to get an idle connection - drop guard before any await
        let mut idle_conn = {
            let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
            idle.pop()
        };

        if let Some(mut conn) = idle_conn.take() {
            // Check if connection is still healthy (guard already dropped)
            if conn.query("SELECT 1 FROM DUAL").await.is_ok() {
                return Ok(PooledConnection {
                    conn: Some(conn),
                    semaphore: self.semaphore.clone(),
                    return_to_pool: Some(self.idle.clone()),
                });
            }
            // Connection is dead, fall through to create new one
        }

        // Create a new connection
        let mut conn = Client::new(&self.host, self.port);
        conn.connect(&self.user, &self.pass).await?;

        Ok(PooledConnection {
            conn: Some(conn),
            semaphore: self.semaphore.clone(),
            return_to_pool: Some(self.idle.clone()),
        })
    }

    /// Get the maximum pool size.
    pub fn max_size(&self) -> usize {
        self.config.max_size
    }

    /// Get the current number of available semaphore permits (idle + available slots).
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

impl std::fmt::Debug for Pool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pool")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("max_size", &self.config.max_size)
            .field("min_idle", &self.config.min_idle)
            .field("available_permits", &self.available_permits())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert_eq!(config.max_size, 10);
        assert_eq!(config.min_idle, 1);
        assert_eq!(config.wait_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_pool_new() {
        let pool = Pool::new("127.0.0.1", 5236, "SYSDBA", "SYSDBA", PoolConfig::default());
        assert_eq!(pool.max_size(), 10);
        assert!(pool.available_permits() > 0);
    }

    #[test]
    fn test_pool_clone() {
        let pool = Pool::new("127.0.0.1", 5236, "SYSDBA", "SYSDBA", PoolConfig::default());
        let pool2 = pool.clone();
        assert_eq!(pool.max_size(), pool2.max_size());
    }
}
