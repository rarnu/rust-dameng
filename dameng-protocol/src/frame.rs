//! 64-byte frame header for Dameng protocol messages.
//!
//! Every message sent between client and server is wrapped in a fixed-size
//! 64-byte frame header followed by the variable-length payload.

use bytes::{Buf, BufMut, BytesMut};

use crate::error::{Error, Result};

/// The size of the frame header in bytes.
pub const FRAME_HEADER_SIZE: usize = 64;

/// DM protocol frame header (64 bytes).
///
/// Layout:
/// ```text
/// Offset  Size  Field
/// 0       4     Version (LE u32, always 0)
/// 4       2     MsgType (LE u16)
/// 6       2     Handle (LE u16)
/// 8       4     Reserved (u32)
/// 12      4     Reserved (u32)
/// 16      2     PayloadLen (LE u16)
/// 18      16    Reserved (zeros)
/// 34      2     Reserved (LE u16)
/// 36      4     Checksum (LE u32)
/// 40      24    Reserved (zeros)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Message type identifier.
    pub msg_type: u16,
    /// Statement/connection handle.
    pub handle: u16,
    /// Length of the payload following this header.
    pub payload_len: u16,
}

impl Frame {
    /// Create a new frame header.
    pub fn new(msg_type: u16, handle: u16, payload_len: u16) -> Self {
        Self {
            msg_type,
            handle,
            payload_len,
        }
    }

    /// Parse a frame header from a buffer.
    ///
    /// Returns `Err(Error::Incomplete)` if fewer than 64 bytes are available.
    pub fn parse(buf: &mut BytesMut) -> Result<Self> {
        if buf.len() < FRAME_HEADER_SIZE {
            return Err(Error::Incomplete);
        }

        let _version = buf.get_u32_le(); // always 0
        let msg_type = buf.get_u16_le();
        let handle = buf.get_u16_le();
        let _reserved = buf.get_u32_le();
        let _reserved = buf.get_u32_le();
        let payload_len = buf.get_u16_le();

        // Skip remaining reserved fields
        buf.advance(42);

        Ok(Frame {
            msg_type,
            handle,
            payload_len,
        })
    }

    /// Encode this frame header into a `BytesMut` buffer.
    pub fn encode(&self) -> BytesMut {
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE);

        buf.put_u32_le(0); // version
        buf.put_u16_le(self.msg_type);
        buf.put_u16_le(self.handle);
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u16_le(self.payload_len);
        buf.put_bytes(0, 46); // remaining reserved fields (64 - 18 = 46)

        debug_assert_eq!(buf.len(), FRAME_HEADER_SIZE);
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_new_and_fields() {
        let frame = Frame::new(200, 0, 82);
        assert_eq!(frame.msg_type, 200);
        assert_eq!(frame.handle, 0);
        assert_eq!(frame.payload_len, 82);
    }

    #[test]
    fn test_frame_encode_size() {
        let frame = Frame::new(1, 0, 59);
        let encoded = frame.encode();
        assert_eq!(encoded.len(), FRAME_HEADER_SIZE);
    }

    #[test]
    fn test_frame_roundtrip() {
        let original = Frame::new(5, 42, 128);
        let mut encoded = original.encode();
        let parsed = Frame::parse(&mut encoded).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn test_frame_parse_incomplete() {
        let mut buf = BytesMut::from(&[0u8; 32][..]);
        let result = Frame::parse(&mut buf);
        assert!(matches!(result, Err(Error::Incomplete)));
    }

    #[test]
    fn test_frame_encode_fields() {
        let frame = Frame::new(228, 7, 255);
        let encoded = frame.encode();
        let bytes = encoded.freeze();

        assert_eq!(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), 0); // version
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 228); // msg_type
        assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 7); // handle
        assert_eq!(u16::from_le_bytes([bytes[16], bytes[17]]), 255); // payload_len
    }

    #[test]
    fn test_frame_parse_fields() {
        let mut encoded = Frame::new(13, 3, 100).encode();
        let frame = Frame::parse(&mut encoded).unwrap();
        assert_eq!(frame.msg_type, 13);
        assert_eq!(frame.handle, 3);
        assert_eq!(frame.payload_len, 100);
    }
}
