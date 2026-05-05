//! BIND message (type 13) for binding parameters to prepared statements.

use bytes::{BufMut, BytesMut};

/// A single parameter to bind.
#[derive(Debug, Clone)]
pub struct BindParam {
    /// SQL type name (e.g., "INT", "VARCHAR").
    pub type_name: String,
    /// DM type code.
    pub type_code: i32,
    /// Precision for numeric types.
    pub precision: i32,
    /// Scale for numeric types.
    pub scale: i16,
    /// The parameter value as bytes.
    pub value: Vec<u8>,
}

/// Client->Server BIND message (type 13).
///
/// Binds parameters to a prepared statement and optionally fetches results.
#[derive(Debug, Clone)]
pub struct BindMessage {
    /// Whether to fetch results after binding.
    pub fetch_flag: u8,
    /// Parameters to bind.
    pub params: Vec<BindParam>,
}

impl BindMessage {
    /// Create a new bind message.
    pub fn new(fetch: bool, params: Vec<BindParam>) -> Self {
        Self {
            fetch_flag: if fetch { 1 } else { 0 },
            params,
        }
    }

    /// Encode to payload bytes.
    pub fn encode_payload(&self) -> BytesMut {
        let mut buf = BytesMut::new();

        buf.put_u8(self.fetch_flag);
        buf.put_u8(0); // reserved
        buf.put_u16_le(0); // reserved
        buf.put_u16_le(self.params.len() as u16);
        buf.put_u16_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved

        for param in &self.params {
            // Type name
            let tn = param.type_name.as_bytes();
            buf.put_u16_le(tn.len() as u16);
            buf.put_slice(tn);

            // Type code, precision, scale
            buf.put_u32_le(param.type_code as u32);
            buf.put_u32_le(param.precision as u32);
            buf.put_u16_le(param.scale as u16);

            // Value
            buf.put_u16_le(param.value.len() as u16);
            buf.put_u16_le(0); // reserved
            buf.put_slice(&param.value);
        }

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bind_new() {
        let bind = BindMessage::new(true, vec![]);
        assert_eq!(bind.fetch_flag, 1);
        assert!(bind.params.is_empty());
    }

    #[test]
    fn test_bind_encode_with_param() {
        let params = vec![BindParam {
            type_name: "INT".to_string(),
            type_code: 4,
            precision: 0,
            scale: 0,
            value: vec![0xC8, 0x03, 0x00, 0x00], // 1000 as i32 LE
        }];
        let bind = BindMessage::new(true, params);
        let payload = bind.encode_payload();
        assert!(payload.len() > 30);
        // param_count at offset 4-5 (fetch_flag(1)+reserved(1)+reserved(2))
        let param_count = u16::from_le_bytes([payload[4], payload[5]]);
        assert_eq!(param_count, 1, "expected 1 got {}", param_count);
    }

    #[test]
    fn test_bind_encode_no_fetch() {
        let bind = BindMessage::new(false, vec![]);
        assert_eq!(bind.fetch_flag, 0);
        let payload = bind.encode_payload();
        assert_eq!(payload[0], 0);
    }

    #[test]
    fn test_bind_multiple_params() {
        let params = vec![
            BindParam {
                type_name: "INT".to_string(),
                type_code: 4,
                precision: 0,
                scale: 0,
                value: vec![1, 0, 0, 0],
            },
            BindParam {
                type_name: "VARCHAR".to_string(),
                type_code: 3,
                precision: 0,
                scale: 0,
                value: b"test".to_vec(),
            },
        ];
        let bind = BindMessage::new(true, params);
        let payload = bind.encode_payload();
        assert_eq!(u16::from_le_bytes([payload[4], payload[5]]), 2);
    }

    #[test]
    fn test_bind_param_fields() {
        let param = BindParam {
            type_name: "BIGINT".to_string(),
            type_code: 5,
            precision: 19,
            scale: 0,
            value: vec![0; 8],
        };
        assert_eq!(param.type_name, "BIGINT");
        assert_eq!(param.type_code, 5);
    }

    #[test]
    fn test_bind_encode_vchar_param() {
        let params = vec![BindParam {
            type_name: "VARCHAR".to_string(),
            type_code: 3,
            precision: 0,
            scale: 0,
            value: b"BindTest".to_vec(),
        }];
        let bind = BindMessage::new(true, params);
        let payload = bind.encode_payload();
        // Should contain "BindTest" somewhere in the value section
        assert!(payload.windows(8).any(|w| w == b"BindTest"));
    }
}
