# Rust Dameng Database Driver

纯 Rust 实现的达梦数据库 (DM) 驱动，支持同步和异步 (tokio) 两种模式。

## 项目结构

```
rust-dameng/
├── dameng-protocol/    # 协议层实现 (消息编码/解码)
├── dameng-types/       # 类型定义和编解码 (DmValue, DmValueType)
├── dameng/             # 同步客户端 (类似 postgres)
├── tokio-dameng/       # 异步客户端 (类似 tokio-postgres)
├── dameng-macros/      # 过程宏 (FromRow derive + query! macros)
├── integration-test/   # 集成测试
└── examples/           # 使用示例
```

## 快速开始

### 同步客户端

```toml
[dependencies]
dameng = { path = "dameng" }
```

```rust
use dameng::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA")?;

    let rs = client.query("SELECT 1")?;
    for row in &rs.rows {
        if let Ok(val) = row.get_i32(0) {
            println!("Result: {}", val);
        }
    }
    Ok(())
}
```

### 异步客户端 (tokio)

```toml
[dependencies]
tokio-dameng = { path = "tokio-dameng" }
tokio = { version = "1", features = ["full"] }
```

```rust
use tokio_dameng::Client;
use tokio_dameng::QueryBuilderExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA").await?;

    let rs = client.query("SELECT 1").await?;
    for row in &rs.rows {
        if let Ok(val) = row.get_i32(0) {
            println!("Result: {}", val);
        }
    }

    // Query API (sqlx-like)
    let rs = client.query("SELECT 42 AS ANS").fetch_all().await?;
    println!("Rows: {}", rs.len());

    Ok(())
}
```

### 连接池

```rust
use tokio_dameng::{Pool, PoolConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = Pool::new("127.0.0.1", 5236, "SYSDBA", "SYSDBA", PoolConfig::default());

    // Checkout a connection from the pool
    let mut conn = pool.get().await?;
    let rs = conn.query("SELECT 1").await?;
    drop(conn); // Auto-return to pool

    Ok(())
}
```

### SQLx-compatible 宏

```rust
use tokio_dameng::sqlx::{FromRow, QueryAs};

#[derive(FromRow)]
struct User {
    id: i32,
    name: String,
}

let users: Vec<User> = QueryAs::new("SELECT id, name FROM users")
    .fetch_all(&mut client).await?;
```

## 功能特性

- **协议解析**: 完整实现达梦数据库二进制协议 (STARTUP/LOGIN/EXEC/FETCH/COMMIT/ROLLBACK等)
- **同步客户端**: 基于标准 TcpStream 的同步连接
- **异步客户端**: 基于 tokio 的异步连接
- **连接池**: Semaphore + Mutex 实现的轻量级异步连接池 (自动归还 + 健康检查)
- **ResultSet**: 统一的查询结果集，包含列元数据和行数据
- **类型编解码**: INT, BIGINT, SMALLINT, TINYINT, VARCHAR, CHAR, FLOAT, DOUBLE, BIT, BLOB, DECIMAL, DATE, TIME, TIMESTAMP 等类型支持
- **事务支持**: BEGIN/COMMIT/ROLLBACK + auto-commit 自动提交
- **SQLx 兼容层**: `#[derive(FromRow)]` + `Query`/`QueryAs`/`QueryScalar` + `dameng_query!`/`dameng_query_as!`/`dameng_query_scalar!` 宏
- **参数绑定**: 安全字符串插值 (INT/VARCHAR/TIMESTAMP/BLOB/NULL 等)

## 示例列表

| 示例 | 描述 |
|------|------|
| `basic_query` | 同步客户端基础查询 |
| `async_query` | 异步客户端 + Query API |
| `crud` | 完整 CRUD + 事务 (BEGIN/COMMIT/ROLLBACK) |
| `parameter_binding` | INT/VARCHAR/TIMESTAMP 参数绑定 (INSERT/UPDATE/DELETE/SELECT) |
| `join_queries` | LEFT JOIN / 三表 JOIN / 聚合 / 子查询 / EXISTS |
| `data_types` | INT/VARCHAR/TIMESTAMP/NULL/COUNT/复合主键类型覆盖 |
| `real_param_binding` | 真正的 execute_with_params 参数绑定 (INT/VARCHAR/TIMESTAMP) |

## 运行示例

```bash
# 设置环境变量
export DM_HOST=127.0.0.1
export DM_PORT=5236
export DM_USER=SYSDBA
export DM_PASS=SYSDBA

# 运行示例
cargo run --package dameng --example basic_query
cargo run --package dameng --example async_query
cargo run --package dameng --example crud
cargo run --package dameng --example parameter_binding
cargo run --package dameng --example join_queries
cargo run --package dameng --example data_types
cargo run --package dameng --example real_param_binding
```

## 运行测试

```bash
# 单元测试 (不需要数据库连接)
cargo test --workspace

# 集成测试 (需要连接到达梦数据库)
cargo run --package dm-integration-test --bin dm-integration-test
```

## 协议细节

协议层基于达梦数据库 8.1.3.62 版本逆向工程实现:
- 帧格式: 64字节头 + 变长载荷
- 消息类型: STARTUP(200), LOGIN(1), READY(3), EXEC(5), OPTIMIZED_PREPARE_EXEC(91), FETCH(7), COMMIT(8), ROLLBACK(9), BIND(13), BIND_EXEC2(90), CLOSE(20), ACK(187) 等
- 加密: XOR 基于服务端 challenge 的简单加密

## 测试统计

- 单元测试: 137 tests passing (dameng-protocol: 90, dameng: 12, tokio-dameng: 26, dameng-macros: 9)
- 集成测试: 19 tests passing (sync CRUD, async CRUD, param binding, transactions)

## License

MIT
