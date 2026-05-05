//! Row representation for query results.

use dameng_types::{DmValue, DmValueType};

/// Column metadata from a query result.
#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub type_code: i32,
    pub type_name: String,
    pub precision: u32,
    pub scale: i16,
    pub nullable: bool,
}

/// A single row of data from a query result.
#[derive(Debug, Clone)]
pub struct Row {
    pub columns: Vec<Column>,
    pub values: Vec<Option<Vec<u8>>>,
}

impl Row {
    /// Get the number of columns in this row.
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    /// Check if the row has no columns.
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Get a value by column index as a decoded DmValue.
    pub fn get(&self, idx: usize) -> Option<DmValue> {
        let data = self.values.get(idx)?.as_ref()?;
        if data.is_empty() {
            return Some(DmValue::Null);
        }
        let ty = self.columns.get(idx)?.type_code;
        let dm_ty = DmValueType::from_type_code(ty)?;
        dameng_types::decode_value(dm_ty, data)
    }

    /// Get an i32 value at the given column index.
    pub fn get_i32(&self, idx: usize) -> Option<i32> {
        match self.get(idx) {
            Some(DmValue::Int(v)) => Some(v),
            _ => None,
        }
    }

    /// Get a String value at the given column index.
    pub fn get_str(&self, idx: usize) -> Option<String> {
        match self.get(idx) {
            Some(DmValue::Text(v)) => Some(v),
            _ => None,
        }
    }

    /// Get an i64 value at the given column index.
    pub fn get_i64(&self, idx: usize) -> Option<i64> {
        match self.get(idx) {
            Some(DmValue::BigInt(v)) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_empty() {
        let row = Row { columns: vec![], values: vec![] };
        assert!(row.is_empty());
        assert_eq!(row.len(), 0);
    }

    #[test]
    fn test_row_with_data() {
        let row = Row {
            columns: vec![Column {
                name: "ID".to_string(),
                type_code: 4, // INT
                type_name: "INT".to_string(),
                precision: 0,
                scale: 0,
                nullable: false,
            }],
            values: vec![Some(vec![42, 0, 0, 0])],
        };
        assert_eq!(row.len(), 1);
        assert!(!row.is_empty());
    }

    #[test]
    fn test_row_get_i32() {
        let row = Row {
            columns: vec![Column {
                name: "ID".to_string(),
                type_code: 4,
                type_name: "INT".to_string(),
                precision: 0,
                scale: 0,
                nullable: false,
            }],
            values: vec![Some(vec![100, 0, 0, 0])],
        };
        assert_eq!(row.get_i32(0), Some(100));
    }

    #[test]
    fn test_row_get_str() {
        let row = Row {
            columns: vec![Column {
                name: "NAME".to_string(),
                type_code: 3,
                type_name: "VARCHAR".to_string(),
                precision: 0,
                scale: 0,
                nullable: true,
            }],
            values: vec![Some(b"Alice".to_vec())],
        };
        assert_eq!(row.get_str(0), Some("Alice".to_string()));
    }

    #[test]
    fn test_row_get_null() {
        let row = Row {
            columns: vec![Column {
                name: "ID".to_string(),
                type_code: 4,
                type_name: "INT".to_string(),
                precision: 0,
                scale: 0,
                nullable: true,
            }],
            values: vec![Some(vec![])],
        };
        match row.get(0) {
            Some(DmValue::Null) => {}
            other => panic!("Expected Null, got {:?}", other),
        }
    }

    #[test]
    fn test_row_get_out_of_range() {
        let row = Row { columns: vec![], values: vec![] };
        assert_eq!(row.get_i32(0), None);
        assert_eq!(row.get_str(0), None);
    }
}
