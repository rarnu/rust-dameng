//! SQLx-compatible query API for async Dameng client.
//!
//! Provides `query()`, `query_as()`, and `query_scalar()` style APIs
//! for type-safe query execution.

use crate::client::Client;
use crate::error::{Error, Result};
use crate::row::ResultSet;
use dameng_protocol::Row;

/// A query builder for executing SQL statements.
///
/// # Example
/// ```ignore
/// let rows: Vec<(i32, String)> = client
///     .query("SELECT id, name FROM users WHERE id = ?")
///     .bind(42)
///     .fetch_all().await?;
/// ```
pub struct Query<'a> {
    client: &'a mut Client,
    sql: String,
    params: Vec<dameng_protocol::message::BindParam>,
}

impl<'a> Query<'a> {
    pub(crate) fn new(client: &'a mut Client, sql: &str) -> Self {
        Self {
            client,
            sql: sql.to_string(),
            params: vec![],
        }
    }

    /// Bind a parameter to the query.
    pub fn bind(mut self, value: dameng_types::DmValue) -> Self {
        // BindParam expects raw bytes, so we need to encode the value
        // For simplicity, support common types directly
        match value.into() {
            dameng_types::DmValue::Int(i) => {
                self.params.push(dameng_protocol::message::BindParam {
                    type_name: "INT".to_string(),
                    type_code: 4,
                    precision: 0,
                    scale: 0,
                    direction: dameng_protocol::message::ParameterDirection::Input,
                    value: Some(i.to_le_bytes().to_vec()),
                });
            }
            dameng_types::DmValue::BigInt(i) => {
                self.params.push(dameng_protocol::message::BindParam {
                    type_name: "BIGINT".to_string(),
                    type_code: 5,
                    precision: 0,
                    scale: 0,
                    direction: dameng_protocol::message::ParameterDirection::Input,
                    value: Some(i.to_le_bytes().to_vec()),
                });
            }
            dameng_types::DmValue::Text(s) => {
                self.params.push(dameng_protocol::message::BindParam {
                    type_name: "VARCHAR".to_string(),
                    type_code: 3,
                    precision: s.len() as i32,
                    scale: 0,
                    direction: dameng_protocol::message::ParameterDirection::Input,
                    value: Some(s.into_bytes()),
                });
            }
            dameng_types::DmValue::Null => {
                self.params.push(dameng_protocol::message::BindParam {
                    type_name: "INT".to_string(),
                    type_code: 4,
                    precision: 0,
                    scale: 0,
                    direction: dameng_protocol::message::ParameterDirection::Input,
                    value: None,
                });
            }
            _ => {
                self.params.push(dameng_protocol::message::BindParam {
                    type_name: "VARCHAR".to_string(),
                    type_code: 3,
                    precision: 0,
                    scale: 0,
                    direction: dameng_protocol::message::ParameterDirection::Input,
                    value: Some(vec![]),
                });
            }
        }
        self
    }

    /// Execute the query and fetch all rows.
    pub async fn fetch_all(self) -> Result<ResultSet> {
        if self.params.is_empty() {
            self.client.execute(&self.sql).await
        } else {
            self.client.execute_with_params(0, &self.sql, &self.params).await
        }
    }

    /// Execute the query and fetch the first row only.
    pub async fn fetch_one(self) -> Result<Row> {
        let rs = self.fetch_all().await?;
        rs.rows.into_iter().next().ok_or(Error::QueryFailed(
            "no rows returned".to_string(),
        ))
    }

    /// Execute the query and get a single scalar value from the first row, first column.
    pub async fn fetch_scalar(self) -> Result<dameng_types::DmValue> {
        let row = self.fetch_one().await?;
        // For scalar, just get the raw bytes and return as best-effort
        match row.values.first().and_then(|v| v.as_ref()) {
            Some(data) if !data.is_empty() => {
                // Try common types
                if data.len() == 4 {
                    Ok(dameng_types::DmValue::Int(i32::from_le_bytes([
                        data[0], data[1], data[2], data[3],
                    ])))
                } else if data.len() == 8 {
                    Ok(dameng_types::DmValue::BigInt(i64::from_le_bytes([
                        data[0], data[1], data[2], data[3],
                        data[4], data[5], data[6], data[7],
                    ])))
                } else {
                    String::from_utf8(data.clone())
                        .map(dameng_types::DmValue::Text)
                        .map_err(|e| Error::DecodeError(e.to_string()))
                }
            }
            _ => Ok(dameng_types::DmValue::Null),
        }
    }

    /// Execute the query and get the first i32 value.
    pub async fn fetch_i32(self) -> Result<i32> {
        let row = self.fetch_one().await?;
        row.get_i32(0).map_err(|e| Error::DecodeError(e.to_string()))
    }

    /// Execute the query and get the first String value.
    pub async fn fetch_str(self) -> Result<String> {
        let row = self.fetch_one().await?;
        row.get_str(0).map_err(|e| Error::DecodeError(e.to_string()))
    }

    /// Execute the query and get the first i64 value.
    pub async fn fetch_i64(self) -> Result<i64> {
        let row = self.fetch_one().await?;
        row.get_i64(0).map_err(|e| Error::DecodeError(e.to_string()))
    }
}

/// Extension trait for Client to provide query() methods.
pub trait QueryBuilderExt {
    /// Create a query builder for the given SQL.
    fn query(&mut self, sql: &str) -> Query;

    /// Create a query builder expecting a single scalar result.
    fn query_scalar<T: Into<dameng_types::DmValue>>(&mut self, sql: &str, value: T) -> Query;
}

impl QueryBuilderExt for Client {
    fn query(&mut self, sql: &str) -> Query {
        Query::new(self, sql)
    }

    fn query_scalar<T>(&mut self, sql: &str, value: T) -> Query
    where
        T: Into<dameng_types::DmValue>,
    {
        Query::new(self, sql).bind(value.into())
    }
}

/// Row extension for extracting typed tuples from rows.
pub trait RowExt {
    /// Extract a (i32, String) tuple from the row.
    fn try_get_i32_str(&self, columns: &[dameng_protocol::Column]) -> Result<(i32, String)>;

    /// Extract a (i32, i32) tuple from the row.
    fn try_get_i32_i32(&self, columns: &[dameng_protocol::Column]) -> Result<(i32, i32)>;

    /// Extract a (String, String) tuple from the row.
    fn try_get_str_str(&self, columns: &[dameng_protocol::Column]) -> Result<(String, String)>;
}

impl RowExt for Row {
    fn try_get_i32_str(&self, columns: &[dameng_protocol::Column]) -> Result<(i32, String)> {
        let a = self.get(0, columns).ok_or(Error::DecodeError("col 0 missing".into()))
            .and_then(|v| match v {
                dameng_types::DmValue::Int(i) => Ok(i),
                _ => Err(Error::DecodeError("expected i32".into())),
            })?;
        let b = self.get(1, columns).ok_or(Error::DecodeError("col 1 missing".into()))
            .and_then(|v| match v {
                dameng_types::DmValue::Text(s) => Ok(s),
                _ => Err(Error::DecodeError("expected String".into())),
            })?;
        Ok((a, b))
    }

    fn try_get_i32_i32(&self, columns: &[dameng_protocol::Column]) -> Result<(i32, i32)> {
        let a = self.get(0, columns).ok_or(Error::DecodeError("col 0 missing".into()))
            .and_then(|v| match v {
                dameng_types::DmValue::Int(i) => Ok(i),
                _ => Err(Error::DecodeError("expected i32".into())),
            })?;
        let b = self.get(1, columns).ok_or(Error::DecodeError("col 1 missing".into()))
            .and_then(|v| match v {
                dameng_types::DmValue::Int(i) => Ok(i),
                _ => Err(Error::DecodeError("expected i32".into())),
            })?;
        Ok((a, b))
    }

    fn try_get_str_str(&self, columns: &[dameng_protocol::Column]) -> Result<(String, String)> {
        let a = self.get(0, columns).ok_or(Error::DecodeError("col 0 missing".into()))
            .and_then(|v| match v {
                dameng_types::DmValue::Text(s) => Ok(s),
                _ => Err(Error::DecodeError("expected String".into())),
            })?;
        let b = self.get(1, columns).ok_or(Error::DecodeError("col 1 missing".into()))
            .and_then(|v| match v {
                dameng_types::DmValue::Text(s) => Ok(s),
                _ => Err(Error::DecodeError("expected String".into())),
            })?;
        Ok((a, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_new() {
        let sql = "SELECT 1";
        let q = Query {
            client: &mut Client::new("localhost", 5236),
            sql: sql.to_string(),
            params: vec![],
        };
        assert_eq!(q.sql, sql);
        assert!(q.params.is_empty());
    }

    #[test]
    fn test_query_bind() {
        let mut client = Client::new("localhost", 5236);
        let q = Query::new(&mut client, "SELECT * FROM t WHERE id = ?");
        let q = q.bind(dameng_types::DmValue::Int(42));
        assert_eq!(q.params.len(), 1);
    }

    #[test]
    fn test_row_ext_i32_str() {
        let columns = vec![
            dameng_protocol::Column {
                name: "ID".to_string(),
                type_code: 4,
                type_name: "INT".to_string(),
                precision: 0, scale: 0, nullable: false,
                display_size: 0,
                table_name: "".to_string(),
                schema_name: "".to_string(),
            },
            dameng_protocol::Column {
                name: "NAME".to_string(),
                type_code: 3,
                type_name: "VARCHAR".to_string(),
                precision: 0, scale: 0, nullable: false,
                display_size: 0,
                table_name: "".to_string(),
                schema_name: "".to_string(),
            },
        ];
        let row = Row {
            row_id: 0,
            values: vec![Some(vec![1, 0, 0, 0]), Some(b"Alice".to_vec())],
        };
        let (id, name) = row.try_get_i32_str(&columns).unwrap();
        assert_eq!(id, 1);
        assert_eq!(name, "Alice");
    }
}
