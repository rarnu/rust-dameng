//! EXEC message (type 5) for preparing and executing SQL statements.
//!
//! Two variants are provided:
//! - `ExecMessage`: simple format (SQL + null terminator) for direct execution.
//! - `ExecMessageV2`: full Go driver-compatible format with auto_commit,
//!   has_result_set, exec_type, max_rows, timeout, and UTF-16 SQL encoding.

use bytes::{BufMut, BytesMut};

/// Client->Server EXEC message (type 5).
///
/// Simple format: SQL string followed by a null terminator.
/// Used for direct execution without parameters.
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

    /// Encode to payload bytes (simple format: SQL + null terminator).
    pub fn encode_payload(&self) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.put_slice(self.sql.as_bytes());
        buf.put_u8(0); // null terminator
        buf
    }
}

/// Client->Server EXEC message for PREPARE (type 5).
///
/// This matches the Go driver's full 64-byte header format used for
/// preparing statements with parameters. The server parses this to
/// extract parameter metadata before the BIND step.
///
/// Wire format (64-byte header + SQL text):
/// ```text
/// Offset  Size  Field
/// 0       4     stmt_id (i32 LE)
/// 4       2     param_count (i16 LE)
/// 6       4     sql_length (i32 LE) - byte length of SQL
/// 10      10    reserved (zeros)
/// 20      1     auto_commit (u8)
/// 21      1     has_result_set (u8)
/// 22      1     reserved (u8)
/// 23      1     exec_flag (u8) = 1
/// 24      1     reserved (u8)
/// 25      2     exec_type (i16 LE) = 0
/// 27      8     max_rows (i64 LE)
/// 35      1     bdta_flag (u8)
/// 36      2     reserved (i16 LE)
/// 38      1     result_set_flag (u8) = 1
/// 39      1     reserved (u8)
/// 40      1     reserved (u8)
/// 41      4     query_timeout (i32 LE)
/// 45      1     inner_exec (u8)
/// 46      1     bind_options (u8) - MsgVersion >= 8
/// 47      2     reserved (zeros)
/// 49      15    reserved (zeros)
/// 64      N     SQL text (encoding depends on server)
/// ```
#[derive(Debug, Clone)]
pub struct PrepareMessage {
    /// Statement handle ID.
    pub stmt_id: u32,
    /// Number of parameter placeholders in the SQL.
    pub param_count: u16,
    /// The SQL string.
    pub sql: String,
    /// Whether this returns a result set (SELECT).
    pub has_result_set: bool,
    /// Auto-commit mode.
    pub auto_commit: bool,
}

impl PrepareMessage {
    /// Create a new PREPARE message.
    pub fn new(stmt_id: u32, sql: &str, param_count: u16, has_result_set: bool) -> Self {
        Self {
            stmt_id,
            param_count,
            sql: sql.to_string(),
            has_result_set,
            auto_commit: true,
        }
    }

    /// Encode to payload bytes (64-byte header + SQL).
    pub fn encode_payload(&self) -> BytesMut {
        let mut buf = BytesMut::new();

        // 0-3: stmt_id
        buf.put_u32_le(self.stmt_id);
        // 4-5: param_count
        buf.put_u16_le(self.param_count);
        // 6-9: sql_length (byte length)
        buf.put_u32_le(self.sql.len() as u32);
        // 10-19: reserved (10 zeros)
        for _ in 0..10 {
            buf.put_u8(0);
        }
        // 20: auto_commit
        buf.put_u8(if self.auto_commit { 1 } else { 0 });
        // 21: has_result_set
        buf.put_u8(if self.has_result_set { 1 } else { 0 });
        // 22: reserved
        buf.put_u8(0);
        // 23: exec_flag
        buf.put_u8(1);
        // 24: reserved
        buf.put_u8(0);
        // 25-26: exec_type (i16 LE) = 0
        buf.put_i16_le(0);
        // 27-34: max_rows (i64 LE) = INT64_MAX (unlimited)
        buf.put_i64_le(i64::MAX);
        // 35: bdta_flag
        buf.put_u8(0);
        // 36-37: reserved (i16 LE)
        buf.put_i16_le(0);
        // 38: result_set_flag
        buf.put_u8(1);
        // 39: reserved
        buf.put_u8(0);
        // 40: reserved
        buf.put_u8(0);
        // 41-44: query_timeout (i32 LE)
        buf.put_i32_le(0);
        // 45: inner_exec
        buf.put_u8(0);
        // 46: bind_options
        buf.put_u8(0);
        // 47-63: reserved (17 zeros)
        for _ in 0..17 {
            buf.put_u8(0);
        }

        // 64+: SQL text as UTF-8 with length prefix (4-byte LE) + null terminator
        // Matches Go driver's Dm_build_1419 format: length + bytes + null
        let sql_bytes = self.sql.as_bytes();
        buf.put_u32_le(sql_bytes.len() as u32); // sql_length (byte count)
        buf.put_slice(sql_bytes);
        buf.put_u8(0); // null terminator

        buf
    }
}

/// Client->Server EXEC message v2 (type 5).
///
/// Full Go driver-compatible format. Supports:
/// - auto_commit control
/// - has_result_set flag (SELECT vs DML)
/// - max_rows for pagination
/// - query timeout
/// - UTF-16 LE encoded SQL string
///
/// Wire format:
/// ```text
/// Offset  Size  Field
/// 0       1     auto_commit (0 or 1)
/// 1       1     has_result_set (1=SELECT, 0=DML)
/// 2       4     reserved (zeros)
/// 6       2     exec_type (u16 LE)
/// 8       8     max_rows (i64 LE) - 0=unlimited
/// 16      1     bdta flag
/// 17      4     timeout (i32 LE) - 0=default
/// 21      2     bind_options (u16 LE)
/// 23      2     sql_length (u16 LE) - number of UTF-16 chars
/// 25      N     SQL text (UTF-16 LE, 2 bytes per char)
/// ```
#[derive(Debug, Clone)]
pub struct ExecMessageV2 {
    /// Auto-commit mode (true = commit after execution).
    pub auto_commit: bool,
    /// Whether this query returns a result set.
    pub has_result_set: bool,
    /// Maximum rows to return (0 = unlimited).
    pub max_rows: i64,
    /// Query timeout in seconds (0 = default).
    pub timeout: i32,
    /// The SQL string.
    pub sql: String,
}

impl ExecMessageV2 {
    /// Create a new v2 exec message with default settings.
    pub fn new(sql: &str) -> Self {
        Self {
            auto_commit: true,
            has_result_set: sql.trim_start().to_uppercase().starts_with("SELECT"),
            max_rows: 0,
            timeout: 0,
            sql: sql.to_string(),
        }
    }

    /// Create for DML statements (INSERT/UPDATE/DELETE).
    pub fn dml(sql: &str) -> Self {
        Self {
            auto_commit: true,
            has_result_set: false,
            max_rows: 0,
            timeout: 0,
            sql: sql.to_string(),
        }
    }

    /// Create for SELECT statements.
    pub fn select(sql: &str) -> Self {
        Self {
            auto_commit: true,
            has_result_set: true,
            max_rows: 0,
            timeout: 0,
            sql: sql.to_string(),
        }
    }

    /// Encode to payload bytes (v2 format with UTF-16 SQL).
    ///
    /// NOTE: For EXEC(5) with stmt_id (PREPARE), the server expects UTF-8
    /// encoded SQL, NOT UTF-16. Use `encode_payload_utf8()` for that case.
    /// This UTF-16 format is used for OPTIMIZED_PREPARE_EXEC(91) only.
    pub fn encode_payload(&self) -> BytesMut {
        let mut buf = BytesMut::new();

        // Header
        buf.put_u8(if self.auto_commit { 1 } else { 0 });
        buf.put_u8(if self.has_result_set { 1 } else { 0 });
        buf.put_u32_le(0); // reserved
        buf.put_u16_le(0); // exec_type

        // Execution options
        buf.put_i64_le(self.max_rows); // max_rows
        buf.put_u8(0); // bdta flag
        buf.put_i32_le(self.timeout); // timeout
        buf.put_u16_le(0); // bind_options

        // SQL as UTF-16 LE
        let sql_utf16: Vec<u16> = self.sql.encode_utf16().collect();
        buf.put_u16_le(sql_utf16.len() as u16); // sql_length (char count)
        for &ch in &sql_utf16 {
            buf.put_u16_le(ch);
        }

        buf
    }

    /// Encode SQL as UTF-8 (for EXEC(5) PREPARE step matching Go driver).
    pub fn encode_payload_utf8(&self) -> BytesMut {
        let mut buf = BytesMut::new();

        // Header
        buf.put_u8(if self.auto_commit { 1 } else { 0 });
        buf.put_u8(if self.has_result_set { 1 } else { 0 });
        buf.put_u32_le(0); // reserved
        buf.put_u16_le(0); // exec_type

        // Execution options
        buf.put_i64_le(self.max_rows); // max_rows
        buf.put_u8(0); // bdta flag
        buf.put_i32_le(self.timeout); // timeout
        buf.put_u16_le(0); // bind_options

        // SQL as UTF-8 with length prefix (4-byte LE) + null terminator
        let sql_bytes = self.sql.as_bytes();
        buf.put_u32_le(sql_bytes.len() as u32); // sql_length (byte count)
        buf.put_slice(sql_bytes);
        buf.put_u8(0); // null terminator

        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ExecMessage (simple) tests ---

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
        let sql_in_payload = &payload[payload.len() - sql.len() - 1..payload.len() - 1];
        assert_eq!(sql_in_payload, sql.as_bytes());
    }

    #[test]
    fn test_exec_encode_null_terminated() {
        let exec = ExecMessage::new("COMMIT", 0);
        let payload = exec.encode_payload();
        assert_eq!(payload[payload.len() - 1], 0);
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
        assert_eq!(payload.len(), "SELECT 1".len() + 1);
    }

    // --- ExecMessageV2 tests ---

    #[test]
    fn test_exec_v2_new_select() {
        let exec = ExecMessageV2::new("SELECT * FROM SAMPLE");
        assert!(exec.auto_commit);
        assert!(exec.has_result_set);
        assert_eq!(exec.max_rows, 0);
    }

    #[test]
    fn test_exec_v2_dml() {
        let exec = ExecMessageV2::dml("DELETE FROM SAMPLE WHERE ID = 1");
        assert!(exec.auto_commit);
        assert!(!exec.has_result_set);
    }

    #[test]
    fn test_exec_v2_select() {
        let exec = ExecMessageV2::select("SELECT ID FROM SAMPLE");
        assert!(exec.has_result_set);
    }

    #[test]
    fn test_exec_v2_encode_minimal() {
        let exec = ExecMessageV2::new("SELECT 1");
        let payload = exec.encode_payload();
        // Header: 25 bytes + UTF-16 SQL (8 chars * 2 = 16 bytes for "SELECT 1")
        assert_eq!(payload.len(), 41);
        assert_eq!(payload[0], 1); // auto_commit
        assert_eq!(payload[1], 1); // has_result_set (SELECT inferred)
    }

    #[test]
    fn test_exec_v2_encode_sql_utf16() {
        let exec = ExecMessageV2::new("AB");
        let payload = exec.encode_payload();
        // After 25-byte header: sql_len(u16) + 'A'(u16 LE) + 'B'(u16 LE)
        assert_eq!(payload[23], 2); // sql_length low byte
        assert_eq!(payload[24], 0); // sql_length high byte
        assert_eq!(payload[25], 0x41); assert_eq!(payload[26], 0); // 'A'
        assert_eq!(payload[27], 0x42); assert_eq!(payload[28], 0); // 'B'
    }

    #[test]
    fn test_exec_v2_encode_cjk_utf16() {
        let exec = ExecMessageV2::new("测");
        let payload = exec.encode_payload();
        // '测' = U+6D4B -> LE bytes: 0x4B 0x6D
        assert_eq!(payload[25], 0x4B);
        assert_eq!(payload[26], 0x6D);
    }

    #[test]
    fn test_exec_v2_no_auto_commit() {
        let mut exec = ExecMessageV2::select("SELECT 1");
        exec.auto_commit = false;
        let payload = exec.encode_payload();
        assert_eq!(payload[0], 0);
    }

    #[test]
    fn test_exec_v2_max_rows() {
        let mut exec = ExecMessageV2::select("SELECT 1");
        exec.max_rows = 100;
        let payload = exec.encode_payload();
        // max_rows at offset 8-15
        assert_eq!(i64::from_le_bytes([payload[8], payload[9], payload[10], payload[11], payload[12], payload[13], payload[14], payload[15]]), 100);
    }

    #[test]
    fn test_exec_v2_timeout() {
        let mut exec = ExecMessageV2::select("SELECT 1");
        exec.timeout = 30;
        let payload = exec.encode_payload();
        // timeout at offset 17-20
        assert_eq!(i32::from_le_bytes([payload[17], payload[18], payload[19], payload[20]]), 30);
    }

    #[test]
    fn test_exec_v2_empty_sql() {
        let exec = ExecMessageV2::new("");
        let payload = exec.encode_payload();
        assert_eq!(payload.len(), 25); // header only
        assert_eq!(u16::from_le_bytes([payload[23], payload[24]]), 0); // sql_length = 0
    }
}
