//! Dameng protocol message types.
//!
//! This module defines all message types used in the DM wire protocol.
//! Messages are organized by their direction (client->server or server->client)
//! and their purpose in the connection lifecycle.

pub mod startup;
pub mod login;
pub mod ready;
pub mod exec;
pub mod bind;
pub mod fetch;
pub mod transaction;
pub mod close;
pub mod response;

pub use startup::*;
pub use login::*;
pub use ready::*;
pub use exec::*;
pub use bind::*;
pub use fetch::*;
pub use transaction::*;
pub use close::*;
pub use response::*;

use bytes::{BufMut, BytesMut};

use crate::frame::Frame;

/// Message type constants.
pub mod msg_type {
    /// STARTUP - Initial connection handshake (client->server)
    pub const STARTUP: u16 = 200;
    /// STARTUP_RESPONSE - Server hello (server->client)
    pub const STARTUP_RESPONSE: u16 = 228;
    /// LOGIN - Send credentials (client->server)
    pub const LOGIN: u16 = 1;
    /// LOGIN_RESPONSE - Authentication result (server->client)
    pub const LOGIN_RESPONSE: u16 = 163;
    /// READY - Send ready/keepalive (client->server)
    pub const READY: u16 = 3;
    /// ACK - Success/generic response (server->client)
    pub const ACK: u16 = 187;
    /// PREPARE/EXEC - Prepare statement or execute (client->server)
    pub const EXEC: u16 = 5;
    /// EXEC_RESPONSE - Statement result (server->client)
    pub const EXEC_RESPONSE: u16 = 0;
    /// BIND - Bind parameters and execute (client->server)
    pub const BIND: u16 = 13;
    /// FETCH - Fetch more rows (client->server)
    pub const FETCH: u16 = 21;
    /// COMMIT - Commit transaction (client->server)
    pub const COMMIT: u16 = 8;
    /// ROLLBACK - Rollback transaction (client->server)
    pub const ROLLBACK: u16 = 7;
    /// CLOSE - Close statement (client->server)
    pub const CLOSE: u16 = 20;
}

/// DM data type codes.
pub mod dm_type {
    pub const BIT: i32 = 1;
    pub const TINYINT: i32 = 2;
    pub const VARCHAR: i32 = 3;
    pub const INT: i32 = 4;
    pub const BIGINT: i32 = 5;
    pub const SMALLINT: i32 = 6;
    pub const FLOAT: i32 = 7;
    pub const DOUBLE: i32 = 8;
    pub const DECIMAL: i32 = 9;
    pub const DATE: i32 = 10;
    pub const TIME: i32 = 11;
    pub const TIMESTAMP: i32 = 12;
    pub const BLOB: i32 = 13;
    pub const CLOB: i32 = 14;
    pub const INTERVAL: i32 = 15;
    pub const CHAR: i32 = 16;
    pub const BINARY: i32 = 17;
    pub const VARBINARY: i32 = 18;
}

/// DM encoding values.
pub mod encoding {
    pub const UTF8: u8 = 1;
    pub const GB18030: u8 = 2;
}

/// Language ID values.
pub mod language {
    pub const EN: u16 = 1;
    pub const CN: u16 = 2;
}

/// Encryption utility functions.
/// DM uses a simple XOR encryption for credentials.
pub mod crypto {
    

    /// Generate encrypted credentials using the server's challenge.
    /// The algorithm XORs the plaintext with the challenge bytes, cycling through.
    pub fn encrypt_with_challenge(plaintext: &[u8], challenge: &[u8], output: &mut [u8]) {
        let challenge_len = challenge.len();
        if challenge_len == 0 {
            output.copy_from_slice(plaintext);
            return;
        }
        for (i, (out, &plain)) in output.iter_mut().zip(plaintext.iter()).enumerate() {
            *out = plain ^ challenge[i % challenge_len];
        }
    }

    /// Build the startup message encrypted random key from server challenge.
    pub fn build_startup_key(challenge: &[u8]) -> [u8; 64] {
        let mut key = [0u8; 64];
        // Generate a simple key pattern based on challenge
        let challenge_len = challenge.len();
        if challenge_len > 0 {
            for i in 0..64 {
                key[i] = challenge[i % challenge_len] ^ (i as u8);
            }
        }
        key
    }
}

/// Build a complete message (frame + payload) and return it as BytesMut.
pub fn build_message(msg_type: u16, handle: u16, payload: &[u8]) -> BytesMut {
    let frame = Frame::new(msg_type, handle, payload.len() as u16);
    let mut result = frame.encode();
    result.put_slice(payload);
    result
}
