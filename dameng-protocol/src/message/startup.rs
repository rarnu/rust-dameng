//! STARTUP message (type 200) and STARTUP_RESPONSE (type 228).
//!
//! The startup phase establishes the initial connection and negotiates
//! encryption parameters.

use bytes::{Buf, BufMut, BytesMut};

use crate::error::Result;
use crate::message::crypto;

/// Client->Server STARTUP message (type 200).
///
/// Sent immediately after TCP connection is established.
/// Payload is 82 bytes.
#[derive(Debug, Clone)]
pub struct StartupMessage {
    /// Client flags.
    pub flags1: u16,
    /// Additional flags.
    pub flags2: u16,
    /// Encrypted random key (64 bytes).
    pub encrypted_key: [u8; 64],
}

impl StartupMessage {
    /// Create a new startup message with the server's challenge.
    pub fn new(server_challenge: &[u8]) -> Self {
        let encrypted_key = crypto::build_startup_key(server_challenge);
        Self {
            flags1: 0x0000,
            flags2: 0x0001,
            encrypted_key,
        }
    }

    /// Encode to payload bytes.
    pub fn encode_payload(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_u16_le(self.flags1);
        buf.put_u16_le(self.flags2);
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u16_le(0); // reserved
        buf.put_slice(&self.encrypted_key);
        buf
    }
}

/// Server->Client STARTUP_RESPONSE message (type 228).
///
/// Contains server version, challenge for encryption, and encoding info.
/// Payload is 112 bytes.
#[derive(Debug, Clone)]
pub struct StartupResponse {
    /// Client major version supported.
    pub client_major: u16,
    /// Client minor version supported.
    pub client_minor: u16,
    /// Server encoding (1=UTF-8, 2=GB18030).
    pub encoding: u8,
    /// Server challenge for encryption (48 bytes).
    pub challenge: [u8; 48],
    /// Server version string.
    pub server_version: String,
    /// Server encryption public key (64 bytes).
    pub encryption_key: [u8; 64],
}

impl StartupResponse {
    /// Parse from payload bytes.
    pub fn parse(buf: &mut impl Buf) -> Result<Self> {
        let _flags1 = buf.get_u16_le();
        let _reserved = buf.get_u32_le();
        let client_major = buf.get_u16_le();
        let client_minor = buf.get_u16_le();
        let _reserved = buf.get_u16_le();
        let encoding = buf.get_u8();

        // Skip 47 bytes to reach challenge at offset 0x0D
        buf.advance(47);

        let mut challenge = [0u8; 48];
        buf.copy_to_slice(&mut challenge);

        let _reserved = buf.get_u32_le();

        // Server version string (12 bytes)
        let mut version_buf = [0u8; 12];
        buf.copy_to_slice(&mut version_buf);
        let server_version = String::from_utf8_lossy(&version_buf).trim_matches('\0').to_string();

        // Encryption public key (64 bytes)
        let mut encryption_key = [0u8; 64];
        buf.copy_to_slice(&mut encryption_key);

        // Skip remaining bytes
        buf.advance(buf.remaining().saturating_sub(0));

        Ok(Self {
            client_major,
            client_minor,
            encoding,
            challenge,
            server_version,
            encryption_key,
        })
    }

    /// Parse from raw bytes slice.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let min_len = 0x99; // 153 bytes expected
        if data.len() < min_len {
            if data.len() < 0x70 {
                return Err(crate::error::Error::Incomplete);
            }
        }

        let _flags1 = u16::from_le_bytes([data[0], data[1]]);
        let _reserved = u32::from_le_bytes([data[2], data[3], data[4], data[5]]);
        let client_major = u16::from_le_bytes([data[6], data[7]]);
        let client_minor = u16::from_le_bytes([data[8], data[9]]);
        let _reserved = u16::from_le_bytes([data[10], data[11]]);
        let encoding = data[12];

        let challenge: [u8; 48] = if data.len() >= 0x3D {
            let mut arr = [0u8; 48];
            arr.copy_from_slice(&data[0x0D..0x3D]);
            arr
        } else {
            [0u8; 48]
        };

        let version_end = data[0x41..0x4D]
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(12);
        let server_version =
            String::from_utf8_lossy(&data[0x41..0x41 + version_end]).to_string();

        let encryption_key: [u8; 64] = if data.len() >= 0x91 {
            let mut arr = [0u8; 64];
            arr.copy_from_slice(&data[0x51..0x91]);
            arr
        } else {
            [0u8; 64]
        };

        Ok(Self {
            client_major,
            client_minor,
            encoding,
            challenge,
            server_version,
            encryption_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_encode_size() {
        let msg = StartupMessage::new(&[]);
        let payload = msg.encode_payload();
        assert_eq!(payload.len(), 78); // 2+2+4+4+2+64
    }

    #[test]
    fn test_startup_with_challenge() {
        let challenge = [0xAB; 48];
        let msg = StartupMessage::new(&challenge);
        let payload = msg.encode_payload();
        // Encrypted key starts at offset 12 (2+2+4+4+2 = 14 bytes header before key)
        // build_startup_key: key[i] = challenge[i % challenge_len] ^ i
        assert_eq!(payload[14], challenge[0] ^ 0); // first byte of key
    }

    #[test]
    fn test_startup_response_from_bytes() {
        // Simulated startup response from protocol capture
        let mut data = [0u8; 153];
        data[6] = 0x01; // client_major = 1
        data[7] = 0x00;
        data[8] = 0x02; // client_minor = 2
        data[9] = 0x00;
        data[12] = 0x01; // UTF-8 encoding
        // Server version at 0x41
        let ver = b"8.1.3.62";
        data[0x41..0x41 + ver.len()].copy_from_slice(ver);
        // Challenge at 0x0D
        for i in 0..48 {
            data[0x0D + i] = 0xBB;
        }
        // Encryption key at 0x51
        for i in 0..64 {
            data[0x51 + i] = 0xCC;
        }

        let resp = StartupResponse::from_bytes(&data).unwrap();
        assert_eq!(resp.client_major, 1);
        assert_eq!(resp.client_minor, 2);
        assert_eq!(resp.encoding, 1); // UTF-8
        assert_eq!(resp.server_version, "8.1.3.62");
    }

    #[test]
    fn test_crypto_encrypt_with_challenge() {
        let plaintext = b"SYSDBA";
        let challenge = [0xFFu8; 16];
        let mut output = vec![0u8; plaintext.len()];
        crypto::encrypt_with_challenge(plaintext, &challenge, &mut output);
        // Each byte XORed with 0xFF
        assert_eq!(output[0], b'S' ^ 0xFF);
        assert_eq!(output[1], b'Y' ^ 0xFF);
    }

    #[test]
    fn test_build_startup_key_empty() {
        let key = crypto::build_startup_key(&[]);
        assert_eq!(key.len(), 64);
        assert_eq!(key[0], 0);
    }

    #[test]
    fn test_build_startup_key_with_challenge() {
        let challenge = [0xAA; 48];
        let key = crypto::build_startup_key(&challenge);
        assert_eq!(key.len(), 64);
        assert_eq!(key[0], 0xAA ^ 0);
        assert_eq!(key[1], 0xAA ^ 1);
    }
}
