//! LOGIN message (type 1) and LOGIN_RESPONSE (type 163).
//!
//! The login phase authenticates the client with the server using
//! encrypted credentials.

use bytes::{BufMut, BytesMut};

use crate::error::Result;
use crate::message::crypto;

/// Client->Server LOGIN message (type 1).
#[derive(Debug, Clone)]
pub struct LoginMessage {
    pub isolation_level: u32,
    pub language_id: u16,
    pub username: String,
    pub password: String,
    pub host: String,
    pub os_name: String,
}

impl LoginMessage {
    /// Create a new login message.
    pub fn new(username: &str, password: &str, host: &str) -> Self {
        Self {
            isolation_level: 0,
            language_id: 1, // EN
            username: username.to_string(),
            password: password.to_string(),
            host: host.to_string(),
            os_name: std::env::consts::OS.to_string(),
        }
    }

    /// Encode to payload bytes with optional encryption.
    pub fn encode_payload(&self, challenge: &[u8]) -> BytesMut {
        let mut buf = BytesMut::new();

        buf.put_u32_le(self.isolation_level);
        buf.put_u32_le(0xFFFF_FFFF); // reserved
        buf.put_u16_le(self.language_id);
        buf.put_u16_le(0);
        buf.put_u32_le(0);
        buf.put_u16_le(0x01E0); // client codepage
        buf.put_u16_le(0);
        buf.put_u16_le(0);
        buf.put_u16_le(0);
        buf.put_bytes(0, 20); // reserved

        // Encrypt and encode username
        let mut username_bytes = [0u8; 128];
        let username_raw = self.username.as_bytes();
        crypto::encrypt_with_challenge(username_raw, challenge, &mut username_bytes[..username_raw.len().min(128)]);
        let username_len = username_raw.len().min(128) as u32;
        buf.put_u32_le(username_len);
        buf.put_slice(&username_bytes);

        // Encrypt and encode password
        let mut password_bytes = [0u8; 128];
        let password_raw = self.password.as_bytes();
        crypto::encrypt_with_challenge(password_raw, challenge, &mut password_bytes[..password_raw.len().min(128)]);
        let password_len = password_raw.len().min(128) as u32;
        buf.put_u32_le(password_len);
        buf.put_slice(&password_bytes);

        // OS name and host
        let os_bytes = self.os_name.as_bytes();
        buf.put_u32_le(os_bytes.len() as u32);
        buf.put_slice(os_bytes);

        let host_bytes = self.host.as_bytes();
        buf.put_u32_le(host_bytes.len() as u32);
        buf.put_slice(host_bytes);
        buf.put_u8(0); // null terminator

        buf
    }
}

/// Server->Client LOGIN_RESPONSE message (type 163).
#[derive(Debug, Clone)]
pub struct LoginResponse {
    pub session_id: u32,
    pub encoding: u8,
    pub server_status: u8,
    pub server_name: String,
    pub username: String,
    pub client_ip: String,
    pub login_datetime: String,
}

impl LoginResponse {
    /// Parse from raw payload bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 64 {
            return Err(crate::error::Error::Incomplete);
        }

        let _flags = u16::from_le_bytes([data[0], data[1]]);
        let session_id = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        let _reserved = u32::from_le_bytes([data[6], data[7], data[8], data[9]]);
        let _reserved = u16::from_le_bytes([data[10], data[11]]);
        let _reserved = u16::from_le_bytes([data[12], data[13]]);
        let encoding = data[14];
        let _reserved = data[15];
        let _reserved = u16::from_le_bytes([data[16], data[17]]);
        let _reserved = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
        let server_status = data[22];

        // Server name at offset 0x38
        let sn_len = u32::from_le_bytes([data[0x38], data[0x39], data[0x3A], data[0x3B]]) as usize;
        let sn_end = (0x3C + sn_len).min(data.len());
        let server_name = String::from_utf8_lossy(&data[0x3C..sn_end])
            .trim_matches('\0')
            .to_string();

        // Username at offset 0x9C (actually at 0x3C + 128 = ~0x9C but real data starts after server_name)
        // From captures: username is right after server name field area
        let un_offset = 0x3C + 128;
        if un_offset + 132 <= data.len() {
            let un_len = u32::from_le_bytes([data[un_offset], data[un_offset + 1], data[un_offset + 2], data[un_offset + 3]]) as usize;
            let un_end = (un_offset + 4 + un_len).min(data.len());
            let username = String::from_utf8_lossy(&data[un_offset + 4..un_end])
                .trim_matches('\0')
                .to_string();

            // IP after username
            let ip_offset = un_offset + 4 + 128;
            if ip_offset + 4 <= data.len() {
                let ip_len = u32::from_le_bytes([data[ip_offset], data[ip_offset + 1], data[ip_offset + 2], data[ip_offset + 3]]) as usize;
                let ip_end = (ip_offset + 4 + ip_len).min(data.len());
                let client_ip = String::from_utf8_lossy(&data[ip_offset + 4..ip_end])
                    .trim_matches('\0')
                    .to_string();

                // Datetime after IP
                let dt_offset = ip_offset + 4 + 48;
                let datetime = if dt_offset + 4 <= data.len() {
                    let dt_len = u32::from_le_bytes([data[dt_offset], data[dt_offset + 1], data[dt_offset + 2], data[dt_offset + 3]]) as usize;
                    let dt_end = (dt_offset + 4 + dt_len).min(data.len());
                    String::from_utf8_lossy(&data[dt_offset + 4..dt_end])
                        .trim_matches('\0')
                        .to_string()
                } else {
                    String::new()
                };

                return Ok(Self {
                    session_id,
                    encoding,
                    server_status,
                    server_name,
                    username,
                    client_ip,
                    login_datetime: datetime,
                });
            }
        }

        Ok(Self {
            session_id,
            encoding,
            server_status,
            server_name,
            username: String::new(),
            client_ip: String::new(),
            login_datetime: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_new() {
        let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
        assert_eq!(login.username, "SYSDBA");
        assert_eq!(login.password, "SYSDBA");
        assert_eq!(login.language_id, 1);
    }

    #[test]
    fn test_login_encode_payload() {
        let challenge = [0xFFu8; 48];
        let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
        let payload = login.encode_payload(&challenge);
        assert!(payload.len() > 64);
        // Check that username is encrypted
        let username_data = &payload[0x3C..0x3C + 6];
        assert_ne!(username_data[0], b'S'); // should be encrypted
    }

    #[test]
    fn test_login_encode_no_challenge() {
        let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
        let payload = login.encode_payload(&[]);
        assert!(payload.len() > 64);
    }

    #[test]
    fn test_login_response_from_bytes() {
        let mut data = [0u8; 256];
        data[2] = 0x40; data[3] = 0x1F; data[4] = 0x00; data[5] = 0x00; // session_id = 0x1F40
        data[14] = 0x01; // UTF-8
        data[22] = 0x01; // server_status
        // Server name at 0x3C
        let sn = b"DMSERVER";
        data[0x38] = sn.len() as u8; // len
        data[0x3C..0x3C + sn.len()].copy_from_slice(sn);

        let resp = LoginResponse::from_bytes(&data).unwrap();
        assert_eq!(resp.encoding, 1);
        assert_eq!(resp.server_status, 1);
    }

    #[test]
    fn test_login_response_incomplete() {
        let data = [0u8; 32];
        let result = LoginResponse::from_bytes(&data);
        assert!(matches!(result, Err(crate::error::Error::Incomplete)));
    }

    #[test]
    fn test_login_isolation_level() {
        let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
        assert_eq!(login.isolation_level, 0);
    }
}
