//! FETCH message (type 21) for retrieving more rows from a result set.

use bytes::{BufMut, BytesMut};

/// Client->Server FETCH message (type 21).
///
/// Requests the next batch of rows from a previously executed query.
#[derive(Debug, Clone)]
pub struct FetchMessage {
    /// Number of rows to fetch.
    pub row_count: u16,
}

impl FetchMessage {
    /// Create a new fetch message requesting the given number of rows.
    pub fn new(row_count: u16) -> Self {
        Self { row_count }
    }

    /// Encode to payload bytes.
    pub fn encode_payload(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_u16_le(self.row_count);
        buf.put_u16_le(0); // reserved
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_new() {
        let fetch = FetchMessage::new(100);
        assert_eq!(fetch.row_count, 100);
    }

    #[test]
    fn test_fetch_encode() {
        let fetch = FetchMessage::new(50);
        let payload = fetch.encode_payload();
        assert_eq!(payload.len(), 4);
        assert_eq!(u16::from_le_bytes([payload[0], payload[1]]), 50);
    }

    #[test]
    fn test_fetch_zero_rows() {
        let fetch = FetchMessage::new(0);
        let payload = fetch.encode_payload();
        assert_eq!(u16::from_le_bytes([payload[0], payload[1]]), 0);
    }

    #[test]
    fn test_fetch_max_u16() {
        let fetch = FetchMessage::new(u16::MAX);
        let payload = fetch.encode_payload();
        assert_eq!(u16::from_le_bytes([payload[0], payload[1]]), u16::MAX);
    }

    #[test]
    fn test_fetch_payload_size() {
        let fetch = FetchMessage::new(1);
        let payload = fetch.encode_payload();
        assert_eq!(payload.len(), 4);
    }

    #[test]
    fn test_fetch_clone() {
        let fetch = FetchMessage::new(42);
        let cloned = fetch.clone();
        assert_eq!(cloned.row_count, 42);
    }
}
