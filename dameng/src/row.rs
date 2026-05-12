//! Row representation for query results.
//!
//! Re-exports unified Column/Row from dameng-protocol with ResultSet wrapper.

pub use dameng_protocol::{Column, Row};

#[cfg(test)]
use dameng_types::DmValue;



/// A query result set containing columns and rows.
#[derive(Debug, Clone)]
pub struct ResultSet {
    /// Column metadata shared across all rows.
    pub columns: Vec<Column>,
    /// Row data.
    pub rows: Vec<Row>,
    /// Result set cursor ID (from the initial query).
    pub cursor_id: i16,
    /// Total row count in the result set (from server).
    pub total_row_count: u64,
}

impl ResultSet {
    /// Create a new empty result set.
    pub fn new() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            cursor_id: 0,
            total_row_count: 0,
        }
    }

    /// Create a result set with the given data.
    pub fn with_data(columns: Vec<Column>, rows: Vec<Row>, cursor_id: i16, total_row_count: u64) -> Self {
        Self {
            columns,
            rows,
            cursor_id,
            total_row_count,
        }
    }

    /// Check if the result set is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Get the number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Get the first row, if any.
    pub fn first(&self) -> Option<&Row> {
        self.rows.first()
    }

    /// Iterate over rows.
    pub fn iter(&self) -> impl Iterator<Item = &Row> {
        self.rows.iter()
    }

    /// Get column metadata by name.
    pub fn column_by_name(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name == name)
    }

    /// Check if there are more rows to fetch.
    pub fn has_more(&self) -> bool {
        self.rows.len() < self.total_row_count as usize
    }

    /// Get the next fetch start position.
    pub fn next_fetch_start(&self) -> usize {
        self.rows.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_row_empty() {
        let row = Row {
            row_id: 0,
            values: vec![],
        };
        assert!(row.is_empty());
        assert_eq!(row.len(), 0);
    }

    #[test]
    fn test_result_set_empty() {
        let rs = ResultSet::new();
        assert!(rs.is_empty());
        assert_eq!(rs.len(), 0);
    }

    #[test]
    fn test_row_get_i32() {
        let row = Row {
            row_id: 0,
            values: vec![Some(vec![100, 0, 0, 0])],
        };
        assert_eq!(row.get_i32(0).unwrap(), 100);
    }

    #[test]
    fn test_row_get_str() {
        let row = Row {
            row_id: 0,
            values: vec![Some(b"Alice".to_vec())],
        };
        assert_eq!(row.get_str(0).unwrap(), "Alice");
    }

    #[test]
    fn test_row_get_dmvlaue() {
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
            lob_tab_id: 0,
            lob_col_id: 0,
        }];
        let row = Row {
            row_id: 0,
            values: vec![Some(vec![100, 0, 0, 0])],
        };
        let val = row.get(0, &columns).unwrap();
        assert_eq!(val, DmValue::Int(100));
    }

    #[test]
    fn test_row_get_out_of_range() {
        let row = Row {
            row_id: 0,
            values: vec![],
        };
        assert!(row.get_i32(0).is_err());
        assert!(row.get_str(0).is_err());
    }
}
