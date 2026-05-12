//! Dameng database async driver with tokio.
//!
//! Provides async connection and query execution against
//! Dameng database servers using tokio runtime.

pub mod client;
pub mod config;
pub mod error;
pub mod pool;
pub mod query;
pub mod row;
pub mod sqlx;

pub use client::Client;
pub use config::ConnectOptions;
pub use error::{Error, Result};
pub use pool::{Pool, PoolConfig, PooledConnection};
pub use query::{QueryBuilderExt, Query, RowExt};
pub use row::ResultSet;
pub use dameng_protocol::{Column, Row};
pub use sqlx::{FromRow, Query as SqlxQuery, QueryAs, QueryScalar};
pub use sqlx::row_ext::RowExt as SqlxRowExt;
