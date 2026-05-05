//! EXEC_RESPONSE (type 0) - Statement execution results.
//!
//! Contains column metadata and row data returned after executing
//! a SELECT statement or binding parameters.


use crate::error::Result;

/// Column metadata from a query result.
#[derive(Debug, Clone)]
pub struct Column {
    /// Column name.
    pub name: String,
    /// DM type code.
    pub type_code: i32,
    /// Type name string (e.g., "INT", "VARCHAR").
    pub type_name: String,
    /// Precision for numeric types.
    pub precision: u32,
    /// Scale for decimal types.
    pub scale: i16,
    /// Whether the column can be NULL.
    pub nullable: bool,
    /// Display size.
    pub display_size: u32,
    /// Table name.
    pub table_name: String,
    /// Schema name.
    pub schema_name: String,
}

/// A single row of data from a query result.
#[derive(Debug, Clone)]
pub struct Row {
    /// Row ID from the database.
    pub row_id: i64,
    /// Column values as raw bytes.
    pub values: Vec<Option<Vec<u8>>>,
}

/// Implementation helper for parsing row values into typed Rust values.
impl Row {
    /// Get an i32 value at the given column index.
    pub fn get_i32(&self, idx: usize) -> Result<i32> {
        let val = self.values.get(idx).and_then(|v| v.as_ref())
            .ok_or(crate::error::Error::DecodeError(format!("column {} is NULL or out of range", idx)))?;
        if val.len() < 4 {
            return Err(crate::error::Error::DecodeError(format!("column {} too short for i32", idx)));
        }
        Ok(i32::from_le_bytes([val[0], val[1], val[2], val[3]]))
    }

    /// Get an i64 value at the given column index.
    pub fn get_i64(&self, idx: usize) -> Result<i64> {
        let val = self.values.get(idx).and_then(|v| v.as_ref())
            .ok_or(crate::error::Error::DecodeError(format!("column {} is NULL or out of range", idx)))?;
        if val.len() < 8 {
            return Err(crate::error::Error::DecodeError(format!("column {} too short for i64", idx)));
        }
        Ok(i64::from_le_bytes([val[0], val[1], val[2], val[3], val[4], val[5], val[6], val[7]]))
        }

    /// Get a String value at the given column index.
    pub fn get_str(&self, idx: usize) -> Result<String> {
        let val = self.values.get(idx).and_then(|v| v.as_ref())
            .ok_or(crate::error::Error::DecodeError(format!("column {} is NULL or out of range", idx)))?;
        String::from_utf8(val.clone())
            .map_err(|e| crate::error::Error::DecodeError(e.to_string()))
    }

    /// Get a f64 value at the given column index.
    pub fn get_f64(&self, idx: usize) -> Result<f64> {
        let val = self.values.get(idx).and_then(|v| v.as_ref())
            .ok_or(crate::error::Error::DecodeError(format!("column {} is NULL or out of range", idx)))?;
        if val.len() < 8 {
            return Err(crate::error::Error::DecodeError(format!("column {} too short for f64", idx)));
        }
        let bytes: [u8; 8] = val[..8].try_into().unwrap();
        Ok(f64::from_le_bytes(bytes))
    }

    /// Check if the value at the given column index is NULL.
    pub fn is_null(&self, idx: usize) -> bool {
        match self.values.get(idx) {
            Some(None) => true,
            Some(Some(v)) => v.len() == 0,
            None => true,
        }
    }

    /// Get the number of columns in this row.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the row has no columns.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Server->Client EXEC_RESPONSE (type 0).
///
/// Contains the result of executing a statement: column metadata,
/// row data, and execution statistics.
#[derive(Debug, Clone)]
pub struct ExecResponse {
    /// Number of columns in the result.
    pub col_count: u16,
    /// Number of rows returned.
    pub row_count: u32,
    /// Column metadata.
    pub columns: Vec<Column>,
    /// Row data.
    pub rows: Vec<Row>,
}

impl ExecResponse {
    /// Parse from raw payload bytes.
    ///
    /// This is the most complex parser - it handles variable-length
    /// column metadata followed by variable-length row data.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 34 {
            return Err(crate::error::Error::Incomplete);
        }

        let _flags = u16::from_le_bytes([data[0], data[1]]);
        let _reserved = u16::from_le_bytes([data[2], data[3]]);
        let _rows_affected = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let _param_count = u16::from_le_bytes([data[8], data[9]]);
        let _reserved = u16::from_le_bytes([data[10], data[11]]);
        let _stmt_handle = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let col_count = u16::from_le_bytes([data[16], data[17]]);
        let _reserved = u16::from_le_bytes([data[18], data[19]]);
        let _reserved = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);

        // Parse column metadata starting at offset 24
        let mut columns = Vec::with_capacity(col_count as usize);
        let mut offset = 24;

        for _ in 0..col_count {
            if offset + 12 > data.len() {
                break;
            }

            // Column info header
            let _col_flag = u8::from_le_bytes([data[offset]]);
            offset += 1;

            let col_name_len = u8::from_le_bytes([data[offset]]) as usize;
            offset += 1;

            let type_name_len = u8::from_le_bytes([data[offset]]) as usize;
            offset += 1;

            let _nullable = data[offset];
            offset += 1;

            // Column name
            if offset + col_name_len > data.len() {
                break;
            }
            let col_name = String::from_utf8_lossy(&data[offset..offset + col_name_len]).to_string();
            offset += col_name_len;

            // Type name
            if offset + type_name_len > data.len() {
                break;
            }
            let type_name = String::from_utf8_lossy(&data[offset..offset + type_name_len]).to_string();
            offset += type_name_len;

            // Table name
            let table_name_len = if offset < data.len() { u8::from_le_bytes([data[offset]]) as usize } else { 0 };
            offset += 1;
            let table_name = if offset + table_name_len <= data.len() {
                let name = String::from_utf8_lossy(&data[offset..offset + table_name_len]).to_string();
                offset += table_name_len;
                name
            } else {
                String::new()
            };

            // Schema name
            let schema_name_len = if offset < data.len() { u8::from_le_bytes([data[offset]]) as usize } else { 0 };
            offset += 1;
            let schema_name = if offset + schema_name_len <= data.len() {
                let name = String::from_utf8_lossy(&data[offset..offset + schema_name_len]).to_string();
                offset += schema_name_len;
                name
            } else {
                String::new()
            };

            // Internal column name (short name)
            let _short_name_len = if offset < data.len() { u8::from_le_bytes([data[offset]]) as usize } else { 0 };
            offset += 1;
            if offset + _short_name_len <= data.len() {
                offset += _short_name_len;
            }

            // Type code, precision, scale, display size
            let type_code = if offset + 4 <= data.len() {
                let code = i32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
                offset += 4;
                code
            } else {
                0
            };

            let precision = if offset + 4 <= data.len() {
                let prec = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
                offset += 4;
                prec
            } else {
                0
            };

            let scale = if offset + 2 <= data.len() {
                let s = i16::from_le_bytes([data[offset], data[offset + 1]]);
                offset += 2;
                s
            } else {
                0
            };

            let _display_size = if offset + 2 <= data.len() {
                let ds = u16::from_le_bytes([data[offset], data[offset + 1]]);
                offset += 2;
                ds as u32
            } else {
                0
            };

            columns.push(Column {
                name: col_name,
                type_code,
                type_name,
                precision,
                scale,
                nullable: true,
                table_name,
                schema_name,
                display_size: 0,
            });
        }

        // Parse rows starting after column metadata
        let mut rows = Vec::new();

        while offset + 10 <= data.len() {
            // Row header: 2 bytes total size + 8 bytes row_id
            let row_size = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;

            if row_size == 0 {
                break;
            }

            let row_id = if offset + 8 <= data.len() {
                let rid = i64::from_le_bytes([
                    data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
                    data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
                ]);
                offset += 8;
                rid
            } else {
                break;
            };

            // Parse column values
            let mut values = Vec::with_capacity(columns.len());

            for _col in &columns {
                if offset + 2 > data.len() {
                    values.push(None);
                    continue;
                }

                let val_len = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
                offset += 2;

                if val_len == 0xFFFF as usize || val_len == 0 {
                    values.push(None);
                } else if offset + val_len <= data.len() {
                    values.push(Some(data[offset..offset + val_len].to_vec()));
                    offset += val_len;
                } else {
                    values.push(None);
                    break;
                }
            }

            rows.push(Row { row_id, values });
        }

        Ok(Self {
            col_count,
            row_count: rows.len() as u32,
            columns,
            rows,
        })
    }

    /// Check if this response contains result rows.
    pub fn has_rows(&self) -> bool {
        !self.rows.is_empty()
    }

    /// Get the number of columns.
    pub fn num_columns(&self) -> usize {
        self.columns.len()
    }

    /// Get the number of rows.
    pub fn num_rows(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_get_i32() {
        let row = Row {
            row_id: 0,
            values: vec![Some(vec![1, 0, 0, 0])],
        };
        assert_eq!(row.get_i32(0).unwrap(), 1);
    }

    #[test]
    fn test_row_get_str() {
        let row = Row {
            row_id: 0,
            values: vec![Some(b"hello".to_vec())],
        };
        assert_eq!(row.get_str(0).unwrap(), "hello");
    }

    #[test]
    fn test_row_is_null() {
        let row = Row {
            row_id: 0,
            values: vec![None, Some(vec![1, 2, 3])],
        };
        assert!(row.is_null(0));
        assert!(!row.is_null(1));
    }

    #[test]
    fn test_row_len() {
        let row = Row {
            row_id: 0,
            values: vec![Some(vec![1]), Some(vec![2]), Some(vec![3])],
        };
        assert_eq!(row.len(), 3);
        assert!(!row.is_empty());
    }

    #[test]
    fn test_row_get_i64() {
        let row = Row {
            row_id: 0,
            values: vec![Some(vec![42, 0, 0, 0, 0, 0, 0, 0])],
        };
        assert_eq!(row.get_i64(0).unwrap(), 42);
    }

    #[test]
    fn test_exec_response_has_rows() {
        let resp = ExecResponse {
            col_count: 0,
            row_count: 0,
            columns: vec![],
            rows: vec![],
        };
        assert!(!resp.has_rows());
    }

    #[test]
    fn test_exec_response_minimal() {
        // Minimal valid header (34 bytes)
        let mut data = vec![0u8; 64];
        data[16] = 0; // col_count = 0
        let resp = ExecResponse::from_bytes(&data).unwrap();
        assert_eq!(resp.col_count, 0);
        assert_eq!(resp.num_columns(), 0);
        assert_eq!(resp.num_rows(), 0);
    }
}
