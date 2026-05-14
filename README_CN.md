# dameng

[![Crates.io](https://img.shields.io/crates/v/dameng.svg)](https://crates.io/crates/dameng)
[![Docs.rs](https://docs.rs/dameng/badge.svg)](https://docs.rs/dameng)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

纯 Rust 实现的[达梦数据库](https://www.dameng.com/) (DM8) 同步驱动，API 设计参考 [rust-postgres](https://docs.rs/postgres/latest/postgres/)。

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
dameng = "0.1"
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
let rs = client.query("SELECT ID, NAME FROM PERSON")?;
for row in rs.iter() {
    let id: i32 = row.get(0).unwrap_or_default();
    let name = row.get_str(1).unwrap_or("<NULL>");
    println!("ID={}, NAME={}", id, name);
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

// 提交 — tx 被消费，Client 借出释放
tx.commit()?;

// Client 可立即继续使用
client.close()?;
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

## 项目结构

```
rust-dameng/
├── dameng/             # 同步驱动（主 crate）
├── dameng-protocol/    # 协议层（消息编解码 + 帧格式）
├── dameng-types/       # 类型系统（DmValue + ToDmValue + 编解码）
├── tokio-dameng/       # 异步驱动（开发中）
├── dameng-macros/      # 过程宏（开发中）
├── integration-test/   # 集成测试
└── examples/           # 使用示例
```

## License

MIT

---

## 发布到 crates.io

此项目为 workspace，包含多个子 crate，需按依赖顺序逐个发布：

### 1. 准备 Cargo.toml

每个子 crate 需要补充以下元数据，并将 `path = "..."` 改为 `version = "0.1"`：

```toml
[package]
repository = "https://github.com/yourname/rust-dameng-ex"
documentation = "https://docs.rs/dameng"
readme = "../README.md"
keywords = ["dameng", "database", "sql", "dm8"]
categories = ["database"]

[dependencies]
dameng-protocol = "0.1"
dameng-types = "0.1"
```

需要修改的文件：`dameng-types/Cargo.toml`、`dameng-protocol/Cargo.toml`、`dameng/Cargo.toml`。

### 2. 按依赖顺序发布

```bash
# 登录
cargo login <your-api-token>

# 干跑检查
cargo publish --dry-run -p dameng-types
cargo publish --dry-run -p dameng-protocol
cargo publish --dry-run -p dameng

# 按依赖顺序发布
cargo publish -p dameng-types
cargo publish -p dameng-protocol
cargo publish -p dameng
```

### 3. 版本管理

- 建议初始版本使用 `0.1.0`，API 稳定后发布 `1.0.0`
- 发布后不可删除，只能发布新版本（`cargo yank` 可以标记为废弃）
- 发布前务必运行 `cargo publish --dry-run` 检查

### 4. 获取 API Token

1. 登录 [crates.io](https://crates.io)，用 GitHub 账号注册
2. 进入 Account Settings → API Tokens → New Token
3. 运行 `cargo login` 粘贴 token
