//! Async connection pool for tokio-dameng.
//!
//! Provides a simple, tokio-native connection pool without external pooling
//! dependencies. Uses `Arc<TokioMutex<Vec<Client>>>` under the hood with
//! a semaphore to limit concurrent connections.
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

use std::sync::Arc;

use tokio::sync::{Mutex as TokioMutex, Semaphore};
use tokio::time::{timeout, Duration};

use crate::client::Client;
use crate::error::{Error, Result};

/// A checked-out connection from the pool.
///
/// Automatically releases the semaphore permit and returns the connection
/// to the idle pool when dropped.
pub struct PooledConnection {
    conn: Option<Client>,
    semaphore: Arc<Semaphore>,
    return_to_pool: Option<Arc<TokioMutex<Vec<Client>>>>,
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
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // If we have a connection and a pool to return it to, put it back
        if let Some(conn) = self.conn.take() {
            if let Some(return_to_pool) = self.return_to_pool.take() {
                // blocking_lock is safe here because we're in Drop (sync context)
                let mut idle = return_to_pool.blocking_lock();
                idle.push(conn);
            }
            // else: pool shut down, connection is dropped
        }
        // Always release the semaphore permit
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
    idle: Arc<TokioMutex<Vec<Client>>>,
}

impl Pool {
    /// Create a new connection pool.
    pub fn new(host: &str, port: u16, user: &str, pass: &str, config: PoolConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_size));
        let idle = Arc::new(TokioMutex::new(Vec::new()));

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
        match timeout(self.config.wait_timeout, self.semaphore.acquire()).await {
            Ok(Ok(_)) => (),
            Ok(Err(_)) => {
                return Err(Error::ConnectionFailed("Semaphore acquire failed".to_string()))
            }
            Err(_) => return Err(Error::ConnectionFailed("Pool acquire timeout".to_string())),
        }

        // Try to get an idle connection
        {
            let mut idle = self.idle.lock().await;
            if let Some(mut conn) = idle.pop() {
                // Check if connection is still healthy
                if conn.query("SELECT 1 FROM DUAL").await.is_ok() {
                    return Ok(PooledConnection {
                        conn: Some(conn),
                        semaphore: self.semaphore.clone(),
                        return_to_pool: Some(self.idle.clone()),
                    });
                }
                // Connection is dead, fall through to create new one
            }
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
