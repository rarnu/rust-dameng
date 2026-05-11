//! SQLx-compatible high-level API for tokio-dameng.
//!
//! Provides `FromRow` trait, `Query`/`QueryAs`/`QueryScalar` builders,
//! and convenience functions for executing typed queries.

pub mod row_ext;

pub use dameng_macros::FromRow;

/// Trait for decoding a database row into a Rust type.
///
/// Automatically derived via `#[derive(FromRow)]`.
pub trait FromRow: Sized {
    /// Construct `Self` from a `Row` and its `Column` metadata.
    fn from_row(row: &dameng_protocol::Row, columns: &[dameng_protocol::Column])
        -> crate::error::Result<Self>;
}

/// A typed query builder for generic results.
pub struct Query<'a> {
    sql: String,
    params: Vec<dameng_protocol::message::BindParam>,
    _marker: std::marker::PhantomData<&'a ()>,
}

/// A typed query builder that decodes rows into a struct type.
pub struct QueryAs<'a, R: FromRow> {
    sql: String,
    params: Vec<dameng_protocol::message::BindParam>,
    _row: std::marker::PhantomData<R>,
    _marker: std::marker::PhantomData<&'a ()>,
}

/// A typed query builder for a single scalar value.
pub struct QueryScalar<'a, S> {
    sql: String,
    params: Vec<dameng_protocol::message::BindParam>,
    _scalar: std::marker::PhantomData<S>,
    _marker: std::marker::PhantomData<&'a ()>,
}

fn make_bind_param(value: dameng_types::DmValue) -> dameng_protocol::message::BindParam {
    match value {
        dameng_types::DmValue::Int(i) => dameng_protocol::message::BindParam {
            type_name: "INT".to_string(),
            type_code: 4,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(i.to_le_bytes().to_vec()),
        },
        dameng_types::DmValue::BigInt(i) => dameng_protocol::message::BindParam {
            type_name: "BIGINT".to_string(),
            type_code: 5,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(i.to_le_bytes().to_vec()),
        },
        dameng_types::DmValue::SmallInt(i) => dameng_protocol::message::BindParam {
            type_name: "SMALLINT".to_string(),
            type_code: 6,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(i.to_le_bytes().to_vec()),
        },
        dameng_types::DmValue::TinyInt(i) => dameng_protocol::message::BindParam {
            type_name: "TINYINT".to_string(),
            type_code: 2,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(i.to_le_bytes().to_vec()),
        },
        dameng_types::DmValue::Float(f) => dameng_protocol::message::BindParam {
            type_name: "FLOAT".to_string(),
            type_code: 7,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(f.to_le_bytes().to_vec()),
        },
        dameng_types::DmValue::Double(d) => dameng_protocol::message::BindParam {
            type_name: "DOUBLE".to_string(),
            type_code: 8,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(d.to_le_bytes().to_vec()),
        },
        dameng_types::DmValue::Text(s) => dameng_protocol::message::BindParam {
            type_name: "VARCHAR".to_string(),
            type_code: 3,
            precision: s.len() as i32,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(s.into_bytes()),
        },
        dameng_types::DmValue::Bytea(b) => dameng_protocol::message::BindParam {
            type_name: "VARBINARY".to_string(),
            type_code: 18,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(b),
        },
        dameng_types::DmValue::Boolean(b) => dameng_protocol::message::BindParam {
            type_name: "BIT".to_string(),
            type_code: 1,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(vec![if b { 1 } else { 0 }]),
        },
        dameng_types::DmValue::Null => dameng_protocol::message::BindParam {
            type_name: "INT".to_string(),
            type_code: 4,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: None,
        },
        dameng_types::DmValue::Decimal(d) => dameng_protocol::message::BindParam {
            type_name: "DECIMAL".to_string(),
            type_code: 9,
            precision: 0,
            scale: 0,
            direction: dameng_protocol::message::ParameterDirection::Input,
            value: Some(d.to_string().into_bytes()),
        },
    }
}

impl<'a> Query<'a> {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            params: vec![],
            _marker: std::marker::PhantomData,
        }
    }

    pub fn bind(mut self, value: impl Into<dameng_types::DmValue>) -> Self {
        self.params.push(make_bind_param(value.into()));
        self
    }

    /// Execute the query and return all rows.
    pub async fn fetch_all(
        self,
        client: &mut crate::client::Client,
    ) -> crate::error::Result<Vec<crate::row::ResultSet>> {
        let rs = if self.params.is_empty() {
            client.query(&self.sql).await?
        } else {
            client.execute_with_params(0, &self.sql, &self.params).await?
        };
        Ok(vec![rs])
    }

    /// Execute the query and return the first row.
    pub async fn fetch_one(
        self,
        client: &mut crate::client::Client,
    ) -> crate::error::Result<crate::row::ResultSet> {
        let rses = self.fetch_all(client).await?;
        rses.into_iter().next().ok_or_else(|| {
            crate::error::Error::QueryFailed("no result set returned".to_string())
        })
    }

    /// Execute the query and get a single i32 from first row, first column.
    pub async fn fetch_i32(
        self,
        client: &mut crate::client::Client,
    ) -> crate::error::Result<i32> {
        let rs = self.fetch_one(client).await?;
        let row = rs.rows.first().ok_or_else(|| {
            crate::error::Error::QueryFailed("no rows returned".to_string())
        })?;
        row.get_i32(0).map_err(|e| {
            crate::error::Error::DecodeError(format!("decode i32 failed: {}", e))
        })
    }

    /// Execute the query and get a single String from first row, first column.
    pub async fn fetch_string(
        self,
        client: &mut crate::client::Client,
    ) -> crate::error::Result<String> {
        let rs = self.fetch_one(client).await?;
        let row = rs.rows.first().ok_or_else(|| {
            crate::error::Error::QueryFailed("no rows returned".to_string())
        })?;
        row.get_str(0).map_err(|e| {
            crate::error::Error::DecodeError(format!("decode string failed: {}", e))
        })
    }
}

impl<'a, R: FromRow> QueryAs<'a, R> {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            params: vec![],
            _row: std::marker::PhantomData,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn bind(mut self, value: impl Into<dameng_types::DmValue>) -> Self {
        self.params.push(make_bind_param(value.into()));
        self
    }

    /// Execute and map all rows to `R`.
    pub async fn fetch_all(
        self,
        client: &mut crate::client::Client,
    ) -> crate::error::Result<Vec<R>> {
        let rs = if self.params.is_empty() {
            client.query(&self.sql).await?
        } else {
            client.execute_with_params(0, &self.sql, &self.params).await?
        };

        let mut results = Vec::new();
        for row in rs.rows {
            let mapped = R::from_row(&row, &rs.columns)?;
            results.push(mapped);
        }
        Ok(results)
    }

    /// Execute and map the first row to `R`.
    pub async fn fetch_one(
        self,
        client: &mut crate::client::Client,
    ) -> crate::error::Result<R> {
        let rows = self.fetch_all(client).await?;
        rows.into_iter()
            .next()
            .ok_or_else(|| crate::error::Error::QueryFailed("no rows returned".to_string()))
    }
}

impl<'a, S> QueryScalar<'a, S> {
    pub fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
            params: vec![],
            _scalar: std::marker::PhantomData,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn bind(mut self, value: impl Into<dameng_types::DmValue>) -> Self {
        self.params.push(make_bind_param(value.into()));
        self
    }

    /// Execute and get the first value as `S`.
    pub async fn fetch_one(
        self,
        client: &mut crate::client::Client,
    ) -> crate::error::Result<S>
    where
        S: std::str::FromStr,
    {
        let rs = if self.params.is_empty() {
            client.query(&self.sql).await?
        } else {
            client.execute_with_params(0, &self.sql, &self.params).await?
        };

        let row = rs.rows.first().ok_or_else(|| {
            crate::error::Error::QueryFailed("no rows returned".to_string())
        })?;

        let val_bytes = row.values.first().and_then(|v| v.as_ref());
        match val_bytes {
            Some(data) if data.len() == 4 => {
                let i = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                Ok(i.to_string().parse().map_err(|_| {
                    crate::error::Error::DecodeError("scalar parse failed".to_string())
                })?)
            }
            Some(data) if data.len() == 8 => {
                let i = i64::from_le_bytes([
                    data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
                ]);
                Ok(i.to_string().parse().map_err(|_| {
                    crate::error::Error::DecodeError("scalar parse failed".to_string())
                })?)
            }
            Some(data) => {
                let s = String::from_utf8_lossy(data);
                s.parse().map_err(|_| {
                    crate::error::Error::DecodeError(format!(
                        "scalar parse failed for: {}",
                        s
                    ))
                })
            }
            None => Err(crate::error::Error::DecodeError(
                "scalar value is NULL".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_new() {
        let q = Query::new("SELECT 1");
        assert_eq!(q.sql, "SELECT 1");
        assert!(q.params.is_empty());
    }

    #[test]
    fn test_query_bind() {
        let q = Query::new("SELECT * FROM t WHERE id = ?")
            .bind(dameng_types::DmValue::Int(42));
        assert_eq!(q.params.len(), 1);
        assert_eq!(q.params[0].type_name, "INT");
    }

    #[test]
    fn test_query_bind_text() {
        let q = Query::new("SELECT * FROM t WHERE name = ?")
            .bind(dameng_types::DmValue::Text(String::from("Alice")));
        assert_eq!(q.params.len(), 1);
        assert_eq!(q.params[0].type_name, "VARCHAR");
    }

    #[test]
    fn test_query_as_new() {
        struct IdRow;
        impl FromRow for IdRow {
            fn from_row(
                _row: &dameng_protocol::Row,
                _columns: &[dameng_protocol::Column],
            ) -> crate::error::Result<Self> {
                Ok(Self)
            }
        }
        let q: QueryAs<IdRow> = QueryAs::new("SELECT id FROM t");
        assert_eq!(q.sql, "SELECT id FROM t");
        assert!(q.params.is_empty());
    }

    #[test]
    fn test_query_as_bind() {
        struct IdRow;
        impl FromRow for IdRow {
            fn from_row(
                _row: &dameng_protocol::Row,
                _columns: &[dameng_protocol::Column],
            ) -> crate::error::Result<Self> {
                Ok(Self)
            }
        }
        let q: QueryAs<IdRow> = QueryAs::new("SELECT id FROM t WHERE id = ?")
            .bind(dameng_types::DmValue::Int(42));
        assert_eq!(q.params.len(), 1);
    }

    #[test]
    fn test_query_scalar_new() {
        let q: QueryScalar<i32> = QueryScalar::new("SELECT COUNT(*) FROM t");
        assert_eq!(q.sql, "SELECT COUNT(*) FROM t");
    }
}
