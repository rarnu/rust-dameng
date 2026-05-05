//! Row representation for tokio-dameng query results.

use dameng_types::{DmValue, DmValueType};

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub type_code: i32,
    pub type_name: String,
    pub precision: u32,
    pub scale: i16,
    pub nullable: bool,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub columns: Vec<Column>,
    pub values: Vec<Option<Vec<u8>>>,
}

impl Row {
    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn get(&self, idx: usize) -> Option<DmValue> {
        let data = self.values.get(idx)?.as_ref()?;
        if data.is_empty() {
            return Some(DmValue::Null);
        }
        let ty = self.columns.get(idx)?.type_code;
        let dm_ty = DmValueType::from_type_code(ty)?;
        dameng_types::decode_value(dm_ty, data)
    }

    pub fn get_i32(&self, idx: usize) -> Option<i32> {
        match self.get(idx) {
            Some(DmValue::Int(v)) => Some(v),
            _ => None,
        }
    }

    pub fn get_str(&self, idx: usize) -> Option<String> {
        match self.get(idx) {
            Some(DmValue::Text(v)) => Some(v),
            _ => None,
        }
    }

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
            values: vec![Some(vec![42, 0, 0, 0])],
        };
        assert_eq!(row.get_i32(0), Some(42));
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
            values: vec![Some(b"Test".to_vec())],
        };
        assert_eq!(row.get_str(0), Some("Test".to_string()));
    }
}
