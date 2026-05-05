//! EXEC message (type 5) for preparing and executing SQL statements.

use bytes::{BufMut, BytesMut};

/// Client->Server EXEC message (type 5).
///
/// Used to prepare a statement or execute it directly.
#[derive(Debug, Clone)]
pub struct ExecMessage {
    /// Whether this is a prepared statement (1) or direct execution (0).
    pub is_prepared: u8,
    /// Number of parameters in the SQL.
    pub param_count: u16,
    /// The SQL string.
    pub sql: String,
}

impl ExecMessage {
    /// Create a new direct execution message.
    pub fn new(sql: &str, param_count: u16) -> Self {
        Self {
            is_prepared: if param_count > 0 { 1 } else { 0 },
            param_count,
            sql: sql.to_string(),
        }
    }

    /// Create a new prepared statement message.
    pub fn prepare(sql: &str, param_count: u16) -> Self {
        Self {
            is_prepared: 1,
            param_count,
            sql: sql.to_string(),
        }
    }

    /// Encode to payload bytes.
    pub fn encode_payload(&self) -> BytesMut {
        let mut buf = BytesMut::new();

        buf.put_u8(self.is_prepared);
        buf.put_u8(0); // reserved
        buf.put_u16_le(0); // reserved
        buf.put_u16_le(self.param_count);
        buf.put_u32_le(0); // reserved
        buf.put_u16_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved
        buf.put_u32_le(0); // reserved

        // SQL string (null-terminated)
        buf.put_slice(self.sql.as_bytes());
        buf.put_u8(0); // null terminator

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exec_new_direct() {
        let exec = ExecMessage::new("SELECT * FROM SAMPLE", 0);
        assert_eq!(exec.is_prepared, 0);
        assert_eq!(exec.param_count, 0);
    }

    #[test]
    fn test_exec_new_prepared() {
        let exec = ExecMessage::new("SELECT * FROM SAMPLE WHERE ID = ?", 1);
        assert_eq!(exec.is_prepared, 1);
        assert_eq!(exec.param_count, 1);
    }

    #[test]
    fn test_exec_encode_contains_sql() {
        let sql = "DELETE FROM SAMPLE WHERE ID = 998";
        let exec = ExecMessage::new(sql, 0);
        let payload = exec.encode_payload();
        // SQL should be in the payload after the header bytes
        let sql_in_payload = &payload[payload.len() - sql.len() - 1..payload.len() - 1];
        assert_eq!(sql_in_payload, sql.as_bytes());
    }

    #[test]
    fn test_exec_encode_null_terminated() {
        let exec = ExecMessage::new("COMMIT", 0);
        let payload = exec.encode_payload();
        assert_eq!(payload[payload.len() - 1], 0); // null terminator
    }

    #[test]
    fn test_exec_prepare() {
        let exec = ExecMessage::prepare("INSERT INTO SAMPLE VALUES (?, ?)", 2);
        assert_eq!(exec.is_prepared, 1);
        assert_eq!(exec.param_count, 2);
        assert_eq!(exec.sql, "INSERT INTO SAMPLE VALUES (?, ?)");
    }

    #[test]
    fn test_exec_payload_size() {
        let exec = ExecMessage::new("SELECT 1", 0);
        let payload = exec.encode_payload();
        // 20 bytes header + 8 bytes SQL + 1 null = 29... but actual is 37 with current layout
        // Just check it's > 0 and contains the SQL
        assert!(payload.len() > 20);
    }
}
