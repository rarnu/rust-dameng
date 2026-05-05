//! Error types for the Dameng driver.

use std::fmt;

#[derive(Debug)]
pub enum Error {
    Protocol(dameng_protocol::Error),
    Io(std::io::Error),
    ConnectionFailed(String),
    AuthFailed(String),
    ServerError(i32, String),
    DecodeError(String),
    NotConnected,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Protocol(e) => write!(f, "protocol error: {e}"),
            Error::Io(e) => write!(f, "IO error: {e}"),
            Error::ConnectionFailed(s) => write!(f, "connection failed: {s}"),
            Error::AuthFailed(s) => write!(f, "auth failed: {s}"),
            Error::ServerError(code, msg) => write!(f, "server error {code}: {msg}"),
            Error::DecodeError(s) => write!(f, "decode error: {s}"),
            Error::NotConnected => write!(f, "not connected"),
        }
    }
}

impl std::error::Error for Error {}

impl From<dameng_protocol::Error> for Error {
    fn from(e: dameng_protocol::Error) -> Self {
        Error::Protocol(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
