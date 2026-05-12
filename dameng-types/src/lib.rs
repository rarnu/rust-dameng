//! Dameng database type definitions and conversions.
//!
//! This crate provides type mappings between Dameng database types
//! and Rust native types, along with encoding/decoding utilities.

use std::str::FromStr;

pub mod encoding;
pub use encoding::{ServerEncoding, encode_to_server, decode_from_server};

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
    /// Raw NBLOB_HEAD bytes from DM server (may be >16 for new LOB format).
    pub raw: Vec<u8>,
    /// Whether this is a CLOB (true) or BLOB (false).
    pub is_clob: bool,
    /// Table ID from column metadata or NBLOB_HEAD extended section.
    /// Used by LOBREAD protocol to locate the LOB data on the server.
    pub tab_id: i32,
    /// Column ID from column metadata.
    /// Used by LOBREAD protocol to locate the LOB data on the server.
    pub col_id: i16,
}

impl LobLocator {
    /// NBLOB_HEAD offsets (matching dm_go constants).
    /// NBLOB_HEAD_IN_ROW_FLAG = 0 (1 byte)
    /// NBLOB_HEAD_BLOBID = 1 (8 bytes)
    /// NBLOB_HEAD_BLOB_LEN = 9 (4 bytes)
    /// NBLOB_HEAD_OUTROW_GROUPID = 13 (2 bytes - USINT)
    /// NBLOB_HEAD_OUTROW_FILEID = 15 (2 bytes - USINT)
    /// NBLOB_HEAD_OUTROW_PAGENO = 17 (4 bytes - ULINT)
    /// NBLOB_EX_HEAD_TABLE_ID = 21 (4 bytes - ULINT)
    /// NBLOB_EX_HEAD_COL_ID = 25 (2 bytes - USINT)
    /// NBLOB_EX_HEAD_ROW_ID = 27 (8 bytes - DDWORD)
    /// NBLOB_EX_HEAD_FPA_GRPID = 35 (2 bytes - USINT)
    /// NBLOB_EX_HEAD_FPA_FILEID = 37 (2 bytes - USINT)
    /// NBLOB_EX_HEAD_FPA_PAGENO = 39 (4 bytes - ULINT)
    const IN_ROW_FLAG: usize = 0;
    const BLOBID: usize = 1;
    const BLOB_LEN: usize = 9;
    const GROUPID: usize = 13;
    const FILEID: usize = 15;
    const PAGENO: usize = 17;
    const EX_TABLE_ID: usize = 21;
    const EX_COL_ID: usize = 25;
    const EX_ROW_ID: usize = 27;
    const EX_FPA_GRPID: usize = 35;
    const EX_FPA_FILEID: usize = 37;
    const EX_FPA_PAGENO: usize = 39;

    /// Create a LOB locator from NBLOB_HEAD raw bytes returned by DM server.
    ///
    /// NBLOB_HEAD layout (out-of-row):
    /// - Off 0:  in_row_flag (1 byte, 0x02 = out-of-row)
    /// - Off 1:  blob_id (8 bytes LE i64)
    /// - Off 9:  group_id (2 bytes LE i16)
    /// - Off 11: file_id (2 bytes LE i16)
    /// - Off 13: page_no (4 bytes LE i32)
    /// - Off 17: (extended section if present)
    /// - Off 21: tab_id (4 bytes LE i32)
    /// - Off 25: col_id (2 bytes LE i16)
    /// - Off 27: row_id (8 bytes LE i64)
    ///
    /// tab_id and col_id can also come from the column metadata in the
    /// EXEC_RESPONSE header (parsed separately), in which case use
    /// `with_tab_col_id()` to set them.
    pub fn from_nblob_head(data: Vec<u8>, is_clob: bool) -> Self {
        let mut tab_id = 0;
        let mut col_id = 0;

        // Try to extract tab_id/col_id from extended NBLOB_HEAD section
        if data.len() >= 29 {
            tab_id = i32::from_le_bytes([
                data[Self::EX_TABLE_ID],
                data[Self::EX_TABLE_ID + 1],
                data[Self::EX_TABLE_ID + 2],
                data[Self::EX_TABLE_ID + 3],
            ]);
            col_id = i16::from_le_bytes([
                data[Self::EX_COL_ID],
                data[Self::EX_COL_ID + 1],
            ]);
        }

        Self {
            raw: data,
            is_clob,
            tab_id,
            col_id,
        }
    }

    /// Set tab_id/col_id from column metadata (overrides NBLOB_HEAD values).
    /// This is called by the response parser after reading the column header.
    pub fn with_tab_col_id(mut self, tab_id: i32, col_id: i16) -> Self {
        self.tab_id = tab_id;
        self.col_id = col_id;
        self
    }

    /// Get the lob_flag value: 0 = BLOB (byte), 1 = CLOB (char).
    pub fn lob_flag(&self) -> u8 {
        if self.is_clob { 1 } else { 0 }
    }

    /// Get the blob_id from the NBLOB_HEAD format (offset 1, 8 bytes LE).
    pub fn blob_id(&self) -> i64 {
        if self.raw.len() >= Self::BLOBID + 8 {
            let bytes: [u8; 8] = self.raw[Self::BLOBID..Self::BLOBID + 8].try_into().unwrap();
            i64::from_le_bytes(bytes)
        } else {
            0
        }
    }

    /// Get the group ID for out-of-row locators (offset 13, 2 bytes LE i16).
    pub fn group_id(&self) -> i16 {
        if self.raw.len() >= Self::GROUPID + 2 {
            i16::from_le_bytes([
                self.raw[Self::GROUPID],
                self.raw[Self::GROUPID + 1],
            ])
        } else {
            -1
        }
    }

    /// Get the file ID for out-of-row locators (offset 15, 2 bytes LE i16).
    pub fn file_id(&self) -> i16 {
        if self.raw.len() >= Self::FILEID + 2 {
            i16::from_le_bytes([
                self.raw[Self::FILEID],
                self.raw[Self::FILEID + 1],
            ])
        } else {
            -1
        }
    }

    /// Get the page number for out-of-row locators (offset 17, 4 bytes LE i32).
    pub fn page_no(&self) -> i32 {
        if self.raw.len() >= Self::PAGENO + 4 {
            let bytes: [u8; 4] = self.raw[Self::PAGENO..Self::PAGENO + 4].try_into().unwrap();
            i32::from_le_bytes(bytes)
        } else {
            -1
        }
    }

    /// Get the row_id from extended section (offset 27, 8 bytes LE i64).
    pub fn row_id(&self) -> i64 {
        if self.raw.len() >= Self::EX_ROW_ID + 8 {
            let bytes: [u8; 8] = self.raw[Self::EX_ROW_ID..Self::EX_ROW_ID + 8].try_into().unwrap();
            i64::from_le_bytes(bytes)
        } else {
            0
        }
    }

    /// Get the extended group ID (offset 35, 2 bytes LE i16).
    pub fn ex_group_id(&self) -> i16 {
        if self.raw.len() >= Self::EX_FPA_GRPID + 2 {
            i16::from_le_bytes([
                self.raw[Self::EX_FPA_GRPID],
                self.raw[Self::EX_FPA_GRPID + 1],
            ])
        } else {
            0
        }
    }

    /// Get the extended file ID (offset 37, 2 bytes LE i16).
    pub fn ex_file_id(&self) -> i16 {
        if self.raw.len() >= Self::EX_FPA_FILEID + 2 {
            i16::from_le_bytes([
                self.raw[Self::EX_FPA_FILEID],
                self.raw[Self::EX_FPA_FILEID + 1],
            ])
        } else {
            0
        }
    }

    /// Get the extended page number (offset 39, 4 bytes LE i32).
    pub fn ex_page_no(&self) -> i32 {
        if self.raw.len() >= Self::EX_FPA_PAGENO + 4 {
            let bytes: [u8; 4] = self.raw[Self::EX_FPA_PAGENO..Self::EX_FPA_PAGENO + 4]
                .try_into()
                .unwrap();
            i32::from_le_bytes(bytes)
        } else {
            0
        }
    }

    /// Check if extended section is present (NewLobFlag).
    pub fn has_extended(&self) -> bool {
        self.raw.len() >= Self::EX_TABLE_ID + 4
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
///
/// # Arguments
/// * `ty` - The DM value type
/// * `data` - The raw bytes to decode
/// * `lob_meta` - Optional LOB column metadata (tab_id, col_id). Used to populate
///   the LobLocator when decoding out-of-row BLOB/CLOB values.
pub fn decode_value(ty: DmValueType, data: &[u8], lob_meta: Option<(i32, i16)>) -> Option<DmValue> {
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
            // DM returns NBLOB_HEAD format for CLOB values:
            // - in_row=0x01: inline data follows (13-byte header: flag(1) + blob_id(8) + blob_len(4) + data)
            // - in_row=0x02: out-of-row LOB locator (needs LOBREAD protocol)
            // - legacy: exactly 16 bytes (old LOB_LOCATOR format)
            if data.len() >= 13 && data[0] == 0x01 {
                // Inline LOB data - extract actual content
                let blob_len = if data.len() >= 13 {
                    u32::from_le_bytes([data[9], data[10], data[11], data[12]]) as usize
                } else {
                    0
                };
                if 13 + blob_len <= data.len() {
                    let inline_data = &data[13..13 + blob_len];
                    String::from_utf8(inline_data.to_vec()).ok().map(DmValue::Text)
                } else {
                    None
                }
            } else if data.len() >= 13 && data[0] == 0x02 {
                // Out-of-row CLOB locator
                let mut loc = LobLocator::from_nblob_head(data.to_vec(), true);
                if let Some((tab_id, col_id)) = lob_meta {
                    loc = loc.with_tab_col_id(tab_id, col_id);
                }
                Some(DmValue::LobLocator(loc))
            } else if data.len() == 16 {
                // Legacy 16-byte LOB_LOCATOR format
                let mut loc = LobLocator::from_nblob_head(data.to_vec(), true);
                if let Some((tab_id, col_id)) = lob_meta {
                    loc = loc.with_tab_col_id(tab_id, col_id);
                }
                Some(DmValue::LobLocator(loc))
            } else {
                String::from_utf8(data.to_vec()).ok().map(DmValue::Text)
            }
        }
        DmValueType::BINARY | DmValueType::VARBINARY => {
            Some(DmValue::Bytea(data.to_vec()))
        }
        DmValueType::BLOB => {
            // DM returns NBLOB_HEAD format for BLOB values (same as CLOB):
            // - in_row=0x01: inline data follows
            // - in_row=0x02: out-of-row LOB locator
            if data.len() >= 13 && data[0] == 0x01 {
                // Inline BLOB data
                let blob_len = if data.len() >= 13 {
                    u32::from_le_bytes([data[9], data[10], data[11], data[12]]) as usize
                } else {
                    0
                };
                if 13 + blob_len <= data.len() {
                    Some(DmValue::Bytea(data[13..13 + blob_len].to_vec()))
                } else {
                    None
                }
            } else if data.len() >= 13 && data[0] == 0x02 {
                // Out-of-row BLOB locator
                let mut loc = LobLocator::from_nblob_head(data.to_vec(), false);
                if let Some((tab_id, col_id)) = lob_meta {
                    loc = loc.with_tab_col_id(tab_id, col_id);
                }
                Some(DmValue::LobLocator(loc))
            } else if data.len() == 16 {
                // Legacy 16-byte LOB_LOCATOR format
                let mut loc = LobLocator::from_nblob_head(data.to_vec(), false);
                if let Some((tab_id, col_id)) = lob_meta {
                    loc = loc.with_tab_col_id(tab_id, col_id);
                }
                Some(DmValue::LobLocator(loc))
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
        let decoded = decode_value(DmValueType::INT, &encoded, None).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_encode_decode_bigint() {
        let val = DmValue::BigInt(1000);
        let encoded = encode_value(DmValueType::BIGINT, &val);
        assert_eq!(encoded, vec![0xE8, 0x03, 0, 0, 0, 0, 0, 0]);
        let decoded = decode_value(DmValueType::BIGINT, &encoded, None).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_encode_decode_text() {
        let val = DmValue::Text("hello".to_string());
        let encoded = encode_value(DmValueType::VARCHAR, &val);
        assert_eq!(encoded, b"hello");
        let decoded = decode_value(DmValueType::VARCHAR, &encoded, None).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_encode_decode_bool() {
        let val = DmValue::Boolean(true);
        let encoded = encode_value(DmValueType::BIT, &val);
        assert_eq!(encoded, vec![1]);
        let decoded = decode_value(DmValueType::BIT, &encoded, None).unwrap();
        assert_eq!(decoded, val);
    }

    #[test]
    fn test_decode_empty() {
        let result = decode_value(DmValueType::INT, &[], None);
        assert_eq!(result, Some(DmValue::Null));
    }

    #[test]
    fn test_encode_decode_bytea() {
        let val = DmValue::Bytea(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let encoded = encode_value(DmValueType::BLOB, &val);
        assert_eq!(encoded, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        let decoded = decode_value(DmValueType::BLOB, &encoded, None).unwrap();
        assert_eq!(decoded, val);
    }
}
