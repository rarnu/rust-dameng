//! Dameng database sync driver.
//!
//! Provides synchronous connection and query execution against
//! Dameng database servers.

pub mod client;
pub mod config;
pub mod error;
pub mod row;

pub use client::Client;
pub use config::ConnectOptions;
pub use error::{Error, Result};
pub use row::Row;

// Re-export protocol types needed for parameter binding
pub use dameng_protocol::message::{BindParam, ParameterDirection};
pub use dameng_protocol::message::isolation::{IsolationLevel, SetIsolationMessage};
