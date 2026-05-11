//! Dameng database type definitions and conversions.
//!
//! This crate provides type mappings between Dameng database types
//! and Rust native types, along with encoding/decoding utilities.

use std::str::FromStr;

/// Dameng SQL value type enum.
///
/// Maps DM type codes to Rust types for encoding and decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmValueType {
    BIT,           // 1
    TINYINT,       // 2
    VARCHAR,       // 3
    INT,           // 4
    BIGINT,        // 5
    SMALLINT,      // 6
    FLOAT,         // 7
    DOUBLE,        // 8
    DECIMAL,       // 9
    DATE,          // 10
    TIME,          // 11
    TIMESTAMP,     // 12
    BLOB,          // 13
    CLOB,          // 14
    INTERVAL,      // 15
    CHAR,          // 16
    BINARY,        // 17
    VARBINARY,     // 18
}

impl DmValueType {
    /// Create a DmValueType from a DM type code.
    pub fn from_type_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(DmValueType::BIT),
            2 => Some(DmValueType::TINYINT),
            3 => Some(DmValueType::VARCHAR),
            4 => Some(DmValueType::INT),
            5 => Some(DmValueType::BIGINT),
            6 => Some(DmValueType::SMALLINT),
            7 => Some(DmValueType::FLOAT),
            8 => Some(DmValueType::DOUBLE),
            9 => Some(DmValueType::DECIMAL),
            10 => Some(DmValueType::DATE),
            11 => Some(DmValueType::TIME),
            12 => Some(DmValueType::TIMESTAMP),
            13 => Some(DmValueType::BLOB),
            14 => Some(DmValueType::CLOB),
            15 => Some(DmValueType::INTERVAL),
            16 => Some(DmValueType::CHAR),
            17 => Some(DmValueType::BINARY),
            18 => Some(DmValueType::VARBINARY),
            _ => None,
        }
    }

    /// Get the DM type code for this value type.
    pub fn type_code(self) -> i32 {
        match self {
            DmValueType::BIT => 1,
            DmValueType::TINYINT => 2,
            DmValueType::VARCHAR => 3,
            DmValueType::INT => 4,
            DmValueType::BIGINT => 5,
            DmValueType::SMALLINT => 6,
            DmValueType::FLOAT => 7,
            DmValueType::DOUBLE => 8,
            DmValueType::DECIMAL => 9,
            DmValueType::DATE => 10,
            DmValueType::TIME => 11,
            DmValueType::TIMESTAMP => 12,
            DmValueType::BLOB => 13,
            DmValueType::CLOB => 14,
            DmValueType::INTERVAL => 15,
            DmValueType::CHAR => 16,
            DmValueType::BINARY => 17,
            DmValueType::VARBINARY => 18,
        }
    }

    /// Get the type name string for protocol messages.
    pub fn type_name(self) -> &'static str {
        match self {
            DmValueType::BIT => "BIT",
            DmValueType::TINYINT => "TINYINT",
            DmValueType::VARCHAR => "VARCHAR",
            DmValueType::INT => "INT",
            DmValueType::BIGINT => "BIGINT",
            DmValueType::SMALLINT => "SMALLINT",
            DmValueType::FLOAT => "FLOAT",
            DmValueType::DOUBLE => "DOUBLE",
            DmValueType::DECIMAL => "DECIMAL",
            DmValueType::DATE => "DATE",
            DmValueType::TIME => "TIME",
            DmValueType::TIMESTAMP => "TIMESTAMP",
            DmValueType::BLOB => "BLOB",
            DmValueType::CLOB => "CLOB",
            DmValueType::INTERVAL => "INTERVAL",
            DmValueType::CHAR => "CHAR",
            DmValueType::BINARY => "BINARY",
            DmValueType::VARBINARY => "VARBINARY",
        }
    }
}

/// A decoded DM value.
#[derive(Debug, Clone, PartialEq)]
pub enum DmValue {
    Null,
    Boolean(bool),
    TinyInt(i8),
    SmallInt(i16),
    Int(i32),
    BigInt(i64),
    Float(f32),
    Double(f64),
    Text(String),
    Bytea(Vec<u8>),
    Decimal(rust_decimal::Decimal),
    /// LOB_LOCATOR: DM server returns a 16-byte locator handle when CLOB/BLOB
    /// data exceeds 2048 bytes. The actual content must be fetched via LOBREAD
    /// protocol messages. This variant stores the raw 16-byte locator.
    LobLocator(LobLocator),
}

/// A LOB (Large Object) locator returned by the DM server.
///
/// When CLOB/BLOB data exceeds 2048 bytes, DM returns a 16-byte locator
/// instead of the actual data. The locator contains server-side pointers
/// (table ID, column ID, row ID, group/file/page numbers) that can be
/// used with LOBREAD protocol messages to fetch the actual content.
#[derive(Debug, Clone, PartialEq)]
pub struct LobLocator {
    /// Raw 16-byte locator from DM server.
    pub raw: [u8; 16],
    /// Whether this is a CLOB (true) or BLOB (false).
    pub is_clob: bool,
}

impl LobLocator {
    /// Create a new LOB locator from raw bytes.
    pub fn new(raw: [u8; 16], is_clob: bool) -> Self {
        Self { raw, is_clob }
    }

    /// Check if this is an "in-row" locator (small data, embedded in row).
    /// Go driver checks byte 0 against a flag byte to determine in-row vs out-row.
    pub fn is_in_row(&self) -> bool {
        // NBLOB_HEAD_IN_ROW_FLAG check: Go uses dm_build_1036(offset=0)
        // In-row flag byte is compared against LOB_IN_ROW constant.
        // Based on Go driver: inRow = byte[0] & flag == LOB_IN_ROW
        // For now, 16-byte locators are always out-row by definition.
        false
    }

    /// Get the blobId (8 bytes, big-endian) from the locator.
    pub fn blob_id(&self) -> i64 {
        // NBLOB_HEAD_BLOBID offset in Go driver
        // Based on Go: blobId = Dm_build_1050(value, NBLOB_HEAD_BLOBID)
        // 1050 reads 8 bytes (int64 LE)
        let bytes: [u8; 8] = self.raw[0..8].try_into().unwrap();
        i64::from_le_bytes(bytes)
    }

    /// Get the group ID (4 bytes) for out-row locators.
    pub fn group_id(&self) -> i32 {
        // NBLOB_HEAD_OUTROW_GROUPID offset
        let bytes: [u8; 4] = self.raw[8..12].try_into().unwrap();
        i32::from_le_bytes(bytes)
    }

    /// Get the file ID (4 bytes) for out-row locators.
    pub fn file_id(&self) -> i32 {
        // NBLOB_HEAD_OUTROW_FILEID offset
        let bytes: [u8; 4] = self.raw[12..16].try_into().unwrap();
        i32::from_le_bytes(bytes)
    }
}

impl From<i32> for DmValue {
    fn from(v: i32) -> Self {
        DmValue::Int(v)
    }
}

impl From<i64> for DmValue {
    fn from(v: i64) -> Self {
        DmValue::BigInt(v)
    }
}

impl From<String> for DmValue {
    fn from(v: String) -> Self {
        DmValue::Text(v)
    }
}

impl From<&str> for DmValue {
    fn from(v: &str) -> Self {
        DmValue::Text(v.to_string())
    }
}

impl From<bool> for DmValue {
    fn from(v: bool) -> Self {
        DmValue::Boolean(v)
    }
}

impl From<f64> for DmValue {
    fn from(v: f64) -> Self {
        DmValue::Double(v)
    }
}

impl From<Vec<u8>> for DmValue {
    fn from(v: Vec<u8>) -> Self {
        DmValue::Bytea(v)
    }
}

/// Encode a Rust value to DM protocol bytes.
pub fn encode_value(ty: DmValueType, value: &DmValue) -> Vec<u8> {
    match ty {
        DmValueType::INT => {
            if let DmValue::Int(v) = value {
                v.to_le_bytes().to_vec()
            } else {
                vec![0; 4]
            }
        }
        DmValueType::BIGINT => {
            if let DmValue::BigInt(v) = value {
                v.to_le_bytes().to_vec()
            } else {
                vec![0; 8]
            }
        }
        DmValueType::SMALLINT => {
            if let DmValue::SmallInt(v) = value {
                v.to_le_bytes().to_vec()
            } else {
                vec![0; 2]
            }
        }
        DmValueType::DOUBLE | DmValueType::FLOAT => {
            if let DmValue::Double(v) = value {
                v.to_le_bytes().to_vec()
            } else if let DmValue::Float(v) = value {
                v.to_le_bytes().to_vec()
            } else {
                vec![0; 8]
            }
        }
        DmValueType::BIT => {
            if let DmValue::Boolean(v) = value {
                vec![if *v { 1 } else { 0 }]
            } else {
                vec![0]
            }
        }
        DmValueType::VARCHAR | DmValueType::CHAR | DmValueType::CLOB => {
            if let DmValue::Text(v) = value {
                v.as_bytes().to_vec()
            } else {
                vec![]
            }
        }
        DmValueType::BLOB | DmValueType::BINARY | DmValueType::VARBINARY => {
            if let DmValue::Bytea(v) = value {
                v.clone()
            } else {
                vec![]
            }
        }
        DmValueType::DECIMAL => {
            if let DmValue::Decimal(v) = value {
                v.to_string().as_bytes().to_vec()
            } else {
                vec![]
            }
        }
        DmValueType::TINYINT => {
            if let DmValue::TinyInt(v) = value {
                v.to_le_bytes().to_vec()
            } else {
                vec![0]
            }
        }
        DmValueType::DATE | DmValueType::TIME | DmValueType::TIMESTAMP | DmValueType::INTERVAL => {
            if let DmValue::Text(v) = value {
                v.as_bytes().to_vec()
            } else {
                vec![]
            }
        }
    }
}

/// Decode DM protocol bytes to a Rust value.
pub fn decode_value(ty: DmValueType, data: &[u8]) -> Option<DmValue> {
    if data.is_empty() {
        return Some(DmValue::Null);
    }

    match ty {
        DmValueType::INT => {
            if data.len() >= 4 {
                let v = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Some(DmValue::Int(v))
            } else {
                None
            }
        }
        DmValueType::BIGINT => {
            if data.len() >= 8 {
                let v = i64::from_le_bytes([
                    data[0], data[1], data[2], data[3],
                    data[4], data[5], data[6], data[7],
                ]);
                Some(DmValue::BigInt(v))
            } else {
                None
            }
        }
        DmValueType::SMALLINT => {
            if data.len() >= 2 {
                let v = i16::from_le_bytes([data[0], data[1]]);
                Some(DmValue::SmallInt(v))
            } else {
                None
            }
        }
        DmValueType::DOUBLE => {
            if data.len() >= 8 {
                let bytes: [u8; 8] = data[..8].try_into().ok()?;
                let v = f64::from_le_bytes(bytes);
                Some(DmValue::Double(v))
            } else {
                None
            }
        }
        DmValueType::FLOAT => {
            if data.len() >= 4 {
                let v = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Some(DmValue::Float(v))
            } else {
                None
            }
        }
        DmValueType::BIT => {
            Some(DmValue::Boolean(data[0] != 0))
        }
        DmValueType::VARCHAR | DmValueType::CHAR => {
            String::from_utf8(data.to_vec()).ok().map(DmValue::Text)
        }
        DmValueType::CLOB => {
            // Check for LOB_LOCATOR (16 bytes) - large CLOB data
            if data.len() == 16 {
                let raw: [u8; 16] = data.try_into().ok()?;
                Some(DmValue::LobLocator(LobLocator::new(raw, true)))
            } else {
                String::from_utf8(data.to_vec()).ok().map(DmValue::Text)
            }
        }
        DmValueType::BINARY | DmValueType::VARBINARY => {
            Some(DmValue::Bytea(data.to_vec()))
        }
        DmValueType::BLOB => {
            // Check for LOB_LOCATOR (16 bytes) - large BLOB data
            if data.len() == 16 {
                let raw: [u8; 16] = data.try_into().ok()?;
                Some(DmValue::LobLocator(LobLocator::new(raw, false)))
            } else {
                Some(DmValue::Bytea(data.to_vec()))
            }
        }
        DmValueType::DECIMAL => {
            let s = String::from_utf8_lossy(data);
            rust_decimal::Decimal::from_str(&s).ok().map(DmValue::Decimal)
        }
        DmValueType::TINYINT => {
            Some(DmValue::TinyInt(data[0] as i8))
        }
        DmValueType::DATE | DmValueType::TIME | DmValueType::TIMESTAMP | DmValueType::INTERVAL => {
            // DM stores DATE/TIME/TIMESTAMP/INTERVAL as binary:
            // DATE: 7 bytes (year:2, month:1, day:1, hour:1, min:1, sec:1)
            // TIME: 6 bytes (hour:1, min:1, sec:1, nanosec:4)
            // TIMESTAMP: 11 bytes (year:2, month:1, day:1, hour:1, min:1, sec:1, nanosec:4)
            // If data is text (string), pass through; otherwise decode binary.
            match String::from_utf8(data.to_vec()) {
                Ok(s) => Some(DmValue::Text(s)),
                Err(_) => {
                    // Try binary decode for TIMESTAMP (11 bytes)
                    if ty == DmValueType::TIMESTAMP && data.len() >= 11 {
                        let year = u16::from_be_bytes([data[0], data[1]]) as i32;
                        let month = data[2];
                        let day = data[3];
                        let hour = data[4];
                        let minute = data[5];
                        let second = data[6];
                        let nano = u32::from_be_bytes([data[7], data[8], data[9], data[10]]);
                        let s = if nano > 0 {
                            format!(
                                "{}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}",
                                year, month, day, hour, minute, second, nano
                            )
                        } else {
                            format!(
                                "{}-{:02}-{:02} {:02}:{:02}:{:02}",
                                year, month, day, hour, minute, second
                            )
                        };
                        Some(DmValue::Text(s))
                    } else if ty == DmValueType::DATE && data.len() >= 7 {
                        let year = u16::from_be_bytes([data[0], data[1]]) as i32;
                        let month = data[2];
                        let day = data[3];
                        let hour = data[4];
                        let minute = data[5];
                        let second = data[6];
                        let s = format!(
                            "{}-{:02}-{:02} {:02}:{:02}:{:02}",
                            year, month, day, hour, minute, second
                        );
                        Some(DmValue::Text(s))
                    } else if ty == DmValueType::TIME && data.len() >= 6 {
                        let hour = data[0];
                        let minute = data[1];
                        let second = data[2];
                        let nano = if data.len() >= 10 {
                            u32::from_be_bytes([data[3], data[4], data[5], data[6]])
                        } else {
                            0
                        };
                        let s = if nano > 0 {
                            format!(
                                "{:02}:{:02}:{:02}.{:09}",
                                hour, minute, second, nano
                            )
                        } else {
                            format!("{:02}:{:02}:{:02}", hour, minute, second)
                        };
                        Some(DmValue::Text(s))
                    } else if ty == DmValueType::INTERVAL {
                        // INTERVAL: decode as text representation
                        // DM stores INTERVAL as binary or text; fallback to lossy UTF-8
                        Some(DmValue::Text(String::from_utf8_lossy(data).to_string()))
                    } else {
                        // Fallback: try as UTF-8 lossy
                        Some(DmValue::Text(String::from_utf8_lossy(data).to_string()))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dmvtype_from_type_code() {
        assert_eq!(DmValueType::from_type_code(4), Some(DmValueType::INT));
        assert_eq!(DmValueType::from_type_code(3), Some(DmValueType::VARCHAR));
        assert_eq!(DmValueType::from_type_code(99), None);
    }

    #[test]
    fn test_dmvtype_type_code() {
        assert_eq!(DmValueType::INT.type_code(), 4);
        assert_eq!(DmValueType::BIGINT.type_code(), 5);
        assert_eq!(DmValueType::BIT.type_code(), 1);
    }

    #[test]
    fn test_dmvtype_type_name() {
        assert_eq!(DmValueType::INT.type_name(), "INT");
        assert_eq!(DmValueType::VARCHAR.type_name(), "VARCHAR");
        assert_eq!(DmValueType::TIMESTAMP.type_name(), "TIMESTAMP");
    }

    #[test]
    fn test_encode_decode_int() {
        let val = DmValue::Int(42);
        let encoded = encode_value(DmValueType::INT, &val);
        assert_eq!(encoded, vec![42, 0, 0, 0]);
        let decoded = decode_value(DmValueType::INT, &encoded).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_encode_decode_bigint() {
        let val = DmValue::BigInt(1000);
        let encoded = encode_value(DmValueType::BIGINT, &val);
        assert_eq!(encoded, vec![0xE8, 0x03, 0, 0, 0, 0, 0, 0]);
        let decoded = decode_value(DmValueType::BIGINT, &encoded).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_encode_decode_text() {
        let val = DmValue::Text("hello".to_string());
        let encoded = encode_value(DmValueType::VARCHAR, &val);
        assert_eq!(encoded, b"hello");
        let decoded = decode_value(DmValueType::VARCHAR, &encoded).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_encode_decode_bool() {
        let val = DmValue::Boolean(true);
        let encoded = encode_value(DmValueType::BIT, &val);
        assert_eq!(encoded, vec![1]);
        let decoded = decode_value(DmValueType::BIT, &encoded).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_decode_empty() {
        let result = decode_value(DmValueType::INT, &[]);
        assert_eq!(result, Some(DmValue::Null));
    }

    #[test]
    fn test_encode_decode_bytea() {
        let val = DmValue::Bytea(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let encoded = encode_value(DmValueType::BLOB, &val);
        assert_eq!(encoded, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let decoded = decode_value(DmValueType::BLOB, &encoded).unwrap();
        assert_eq!(decoded, val);
    }
}
