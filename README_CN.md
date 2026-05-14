# dameng

[![Crates.io](https://img.shields.io/crates/v/dameng.svg)](https://crates.io/crates/dameng)
[![Docs.rs](https://docs.rs/dameng/badge.svg)](https://docs.rs/dameng)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

纯 Rust 实现的[达梦数据库](https://www.dameng.com/) (DM8) 驱动。

> [English Documentation](README.md)

## 特性

- **纯 Rust 协议实现** — 完整实现达梦 DM8 二进制协议（STARTUP/LOGIN/EXEC/OPE/FETCH/COMMIT/ROLLBACK）
- **SQLx 风格参数绑定** — `&[&id, &name]` 动态参数，自动类型转换
- **事务支持** — `Transaction` API，Drop 时自动回滚
- **完整的类型支持** — INT, BIGINT, VARCHAR, FLOAT, DOUBLE, DECIMAL, DATE, TIME, TIMESTAMP, BLOB, CLOB 等
- **安全** — 免 SQL 注入的参数化查询
- **TLS 支持** — 可选的 SSL/TLS 加密连接

## 安装

```toml
[dependencies]
dameng = "0.1.0"
```

## 快速开始

### 连接数据库

```rust
use dameng::Client;

let mut client = Client::new("127.0.0.1", 5236);
client.connect("SYSDBA", "SYSDBA")?;
```

### 基础查询

```rust
let rs = client.query("SELECT ID, NAME, ADDRESS FROM PERSON")?;
for row in rs.iter() {
let id: i32 = row.get(0)?;
let name: &str = row.get(1)?;
let address: Option<&str> = row.get(2)?;
println!("ID={}, NAME={}, ADDRESS={:?}", id, name, address);
}
```

### 参数化查询（SQLx 风格）

```rust
let id: i32 = 1;
let name: &str = "Alice";
let rs = client.query_with_params(
    "SELECT * FROM PERSON WHERE ID = ? AND NAME = ?",
    &[&id, &name],
)?;
```

### DML 操作（INSERT / UPDATE / DELETE）

```rust
let affected = client.execute_with_params(
    "INSERT INTO PERSON (ID, NAME, AGE) VALUES (?, ?, ?)",
    &[&1, &"Alice", &25],
)?;
println!("Inserted {} rows", affected);
```

### 事务

```rust
let mut tx = client.transaction()?;

tx.execute_with_params("INSERT INTO PERSON VALUES (?, ?)", &[&1, &"Alice"])?;
tx.execute_with_params("INSERT INTO PERSON VALUES (?, ?)", &[&2, &"Bob"])?;

tx.commit()?;
```

事务内的所有操作作为一个原子单元执行。如果 `Transaction` 被 `Drop` 而未显式 `commit()` / `rollback()`，会自动执行 `ROLLBACK`。

### 完整 CRUD 示例

```rust
use dameng::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA")?;

    // CREATE
    client.execute("CREATE TABLE IF NOT EXISTS users (
        id INT PRIMARY KEY,
        name VARCHAR(100),
        age INT
    )")?;

    // INSERT — 事务批量插入
    let mut tx = client.transaction()?;
    let users = [(1, "Alice", 25i32), (2, "Bob", 30), (3, "Carol", 28)];
    for (id, name, age) in &users {
        tx.execute_with_params(
            "INSERT INTO users VALUES (?, ?, ?)",
            &[&id, &name, &age],
        )?;
    }
    tx.commit()?;

    // SELECT
    let rs = client.query_with_params(
        "SELECT name, age FROM users WHERE age > ? ORDER BY age",
        &[&26i32],
    )?;
    for row in rs.iter() {
        let name = row.get_str(0).unwrap_or("<NULL>");
        let age: i32 = row.get(1).unwrap_or_default();
        println!("{name}: {age}");
    }

    // UPDATE
    client.execute_with_params(
        "UPDATE users SET age = ? WHERE id = ?",
        &[&31i32, &1i32],
    )?;

    // DELETE
    client.execute_with_params("DELETE FROM users WHERE id = ?", &[&3i32])?;

    client.close()?;
    Ok(())
}
```

## API 概览

### Client 方法

| 方法 | 返回 | 说明 |
|------|------|------|
| `Client::new(host, port)` | `Client` | 创建客户端 |
| `connect(username, password)` | `Result<()>` | 连接数据库 |
| `close()` | `Result<()>` | 关闭连接 |
| `transaction()` | `Result<Transaction>` | 开启事务 |
| `execute(sql)` | `Result<u64>` | 执行 DML，返回影响行数 |
| `execute_with_params(sql, params)` | `Result<u64>` | 带参数的 DML |
| `query(sql)` | `Result<ResultSet>` | 执行 SELECT 查询 |
| `query_with_params(sql, params)` | `Result<ResultSet>` | 带参数的 SELECT |
| `begin()` | `Result<()>` | 关闭自动提交（低级 API） |

### Transaction 方法

| 方法 | 返回 | 说明 |
|------|------|------|
| `commit(self)` | `Result<()>` | 提交事务，消费 tx，释放 Client |
| `rollback(self)` | `Result<()>` | 回滚事务，消费 tx，释放 Client |
| `execute(sql)` | `Result<u64>` | 事务内 DML |
| `execute_with_params(sql, params)` | `Result<u64>` | 事务内带参 DML |
| `query(sql)` | `Result<ResultSet>` | 事务内 SELECT |
| `query_with_params(sql, params)` | `Result<ResultSet>` | 事务内带参 SELECT |

### ResultSet 方法

| 方法 | 说明 |
|------|------|
| `iter()` | 返回行迭代器 |
| `columns` | 列元数据 `Vec<Column>` |
| `rows` | 行数据 `Vec<Row>` |
| `total_row_count` | 服务器返回的总行数 |

### QueryRow 方法

| 方法 | 说明 |
|------|------|
| `get::<T>(idx)` | 按类型获取列值（推荐） |
| `get_i32(idx)` / `get_i64(idx)` | 整数获取 |
| `get_str(idx)` | 字符串获取 |
| `get_f64(idx)` | 浮点数获取 |
| `get_opt_str(idx)` | 可空字符串 |

## 类型映射

| Rust 类型 | DM 类型 | ToDmValue |
|-----------|---------|-----------|
| `i8` | TINYINT | `DmValue::TinyInt` |
| `i16` | SMALLINT | `DmValue::SmallInt` |
| `i32` | INT | `DmValue::Int` |
| `i64` | BIGINT | `DmValue::BigInt` |
| `f32` | FLOAT | `DmValue::Float` |
| `f64` | DOUBLE | `DmValue::Double` |
| `bool` | BIT | `DmValue::Boolean` |
| `&str` / `String` | VARCHAR | `DmValue::Text` |
| `Vec<u8>` | VARBINARY | `DmValue::Bytea` |
| `rust_decimal::Decimal` | DECIMAL | — |
| `chrono::NaiveDate` | DATE | — |
| `chrono::NaiveDateTime` | TIMESTAMP | — |

## 连接配置

```rust
use dameng::{Client, ConnectOptions, IsolationLevel};

// 通过 DSN 连接
let opts = ConnectOptions::from_dsn(
    "dm://SYSDBA:SYSDBA@127.0.0.1:5236/?auto_commit=false&isolation_level=serializable"
)?;
let mut client = Client::connect_with(&opts)?;

// 或手动配置
let mut client = Client::new("127.0.0.1", 5236);
client.auto_commit = false;
client.isolation_level = IsolationLevel::Serializable;
client.connect("SYSDBA", "SYSDBA")?;
```

## License

MIT
