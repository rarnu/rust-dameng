//! Row extension trait for extracting typed values by column name.

use crate::error::Result;
use dameng_protocol::{Column, Row};

/// Extract typed values from a row by column name.
pub trait RowExt {
    /// Find the column index for a given name (case-insensitive).
    fn find_column<'a>(&'a self, columns: &'a [Column], name: &str) -> Option<usize>;

    /// Get a value by column name.
    fn get_by_name(&self, columns: &[Column], name: &str) -> Option<dameng_types::DmValue>;
}

impl RowExt for Row {
    fn find_column<'a>(&'a self, columns: &'a [Column], name: &str) -> Option<usize> {
        columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }

    fn get_by_name(&self, columns: &[Column], name: &str) -> Option<dameng_types::DmValue> {
        let idx = self.find_column(columns, name)?;
        self.get(idx, columns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_column() {
        let columns = vec![
            Column {
                name: "ID".to_string(),
                type_code: 4,
                type_name: "INT".to_string(),
                precision: 0,
                scale: 0,
                nullable: false,
                display_size: 0,
                table_name: "".to_string(),
                schema_name: "".to_string(),
            },
            Column {
                name: "NAME".to_string(),
                type_code: 3,
                type_name: "VARCHAR".to_string(),
                precision: 0,
                scale: 0,
                nullable: false,
                display_size: 0,
                table_name: "".to_string(),
                schema_name: "".to_string(),
            },
        ];
        let row = Row {
            row_id: 0,
            values: vec![],
        };
        assert_eq!(row.find_column(&columns, "ID"), Some(0));
        assert_eq!(row.find_column(&columns, "name"), Some(1));
        assert_eq!(row.find_column(&columns, "NONEXIST"), None);
    }

    #[test]
    fn test_get_by_name() {
        let columns = vec![Column {
            name: "ID".to_string(),
            type_code: 4,
            type_name: "INT".to_string(),
            precision: 0,
            scale: 0,
            nullable: false,
            display_size: 0,
            table_name: "".to_string(),
            schema_name: "".to_string(),
        }];
        let row = Row {
            row_id: 0,
            values: vec![Some(vec![42, 0, 0, 0])],
        };
        let val = row.get_by_name(&columns, "ID").unwrap();
        assert_eq!(val, dameng_types::DmValue::Int(42));
    }

    #[test]
    fn test_get_by_name_missing() {
        let columns = vec![];
        let row = Row {
            row_id: 0,
            values: vec![],
        };
        assert!(row.get_by_name(&columns, "NOPE").is_none());
    }
}
