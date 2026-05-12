# 修复路线图 — Rust 达梦驱动完善计划

## 项目状态
- **最后更新**: 2026-05-11
- **当前 commit**: `9909ad5` feat: add LOB_LOCATOR detection for CLOB/BLOB streaming
- **测试状态**: 137 unit tests ✅, 19 integration tests ✅
- **Build**: Clean, 0 warnings ✅

---

## HIGH 优先级

### [🟡] H1: LOB 读取实现 (LOBREAD 协议) — 基础设施完成

**问题**: CLOB/BLOB > 2048 字节时 DM 返回 16 字节 LOB_LOCATOR，但 Rust 驱动无法读取实际数据。

**Go 实现参考**:
- `dm_go/k.go`: `DmBlob.getBytes()` → `blob.connection.Access.dm_build_187(blob, pos, length)`
- `dm_go/l.go`: `DmClob.getSubString()` → 同样调用 dm_build_187
- LOB_LOCATOR = 16 字节二进制句柄
- LOBREAD 协议: 通过 `dm_build_187` 发送 LOB 句柄 + 偏移 + 长度 → 返回数据

**需要实现**:
1. `dameng-protocol/src/message/lob.rs`: LOBREAD/LOBFREE 消息编码
2. `dameng/src/client.rs`: `read_lob(lob_locator: &[u8], pos: i64, length: i32)` 方法
3. `dameng-types/src/lib.rs`: `DmValue::LobLocator(LobLocator)` 变体 ✅
4. `dameng-protocol/src/message/response.rs`: 解析时检测 16 字节 CLOB/BLOB 值 → 返回 LobLocator ✅
5. `dameng/examples/test_lob_locator.rs`: LOB 检测测试示例 ✅

**验收标准**:
- 插入 > 2048 字节的 CLOB 数据，能正确读取回完整内容
- 插入 > 2048 字节的 BLOB 数据，能正确读取回完整内容

---

### [ ] H2: GB18030 ↔ UTF-8 编码转换

**问题**: DM 服务器可能使用 GB18030 编码，Rust 驱动仅使用 UTF-8/lossy 解码。

**Go 实现参考**:
- `dm_go/m.go`: `encode encoding.Encoding` 字段
- 连接时从 LOGIN_RESPONSE 读取服务器编码 (1=UTF-8, 2=GB18030)
- 所有发送/接收的数据经过编码转换

**需要实现**:
1. 添加 `encoding_rs` 或 `charset` crate 依赖
2. `dameng/src/client.rs`: 连接时保存服务器编码
3. `dameng/src/encoding.rs`: GB18030 ↔ UTF-8 转换工具函数
4. 所有 `write_all` 发送前转换, 所有 `read_message` 接收后转换

**验收标准**:
- 服务器返回 GB18030 编码的中文数据能正确解码
- 发送中文 SQL 参数能正确编码

---

## MEDIUM 优先级

### [ ] M1: 事务隔离级别支持

**问题**: Go 驱动支持设置事务隔离级别 (READ COMMITTED 等)，Rust 不支持。

**Go 实现参考**:
- `dm_go/m.go`: `IsoLevel int32` 字段 + `g2dbIsoLevel()` 转换
- 使用 SET_ISOLATION (msg_type=52) 协议消息

**需要实现**:
1. `dameng-protocol/src/message/isolation.rs`: SET_ISOLATION 消息编码
2. `dameng/src/client.rs`: `set_isolation(level: IsolationLevel)` 方法
3. `dameng/src/lib.rs`: `IsolationLevel` 枚举 (ReadUncommitted, ReadCommitted, Serializable, etc)

**验收标准**:
- 能设置 READ COMMITTED/READ UNCOMMITTED/SERIALIZABLE
- DM 正确应用隔离级别

---

### [ ] M2: 连接参数扩展

**问题**: Go 驱动支持 charset/schema/timezone/SSL 等连接参数，Rust 仅支持 host/port/user/pass。

**Go 实现参考**:
- `dm_go/m.go`: DmConnection 大量字段 (charset, schema, timezone, sslEncrypt, MaxRowSize 等)

**需要实现**:
1. `dameng/src/config.rs`: `ConnectOptions` 结构体
   - `charset: Option<&str>`
   - `schema: Option<&str>`
   - `timezone: Option<i16>`
   - `ssl: bool`
   - `max_row_size: Option<i32>`
   - `connect_timeout: Option<Duration>`
2. `dameng/src/client.rs`: `connect_with(config: &ConnectOptions)` 方法
3. DSN 字符串解析: `Client::connect_from_dsn("dm://user:pass@host:port/schema?charset=utf8")`

**验收标准**:
- 能通过配置指定 schema
- 能通过 DSN 字符串连接

---

### [ ] M3: CLOB/BLOB 参数绑定

**问题**: Go 支持 CLOB/BLOB 类型的参数绑定，Rust 仅支持 INT/VARCHAR/TIMESTAMP。

**Go 实现参考**:
- `dm_go/zi.go`: `fromString()` 对 CLOB/BLOB 调用 `string2Clob()`/`bytes2Blob()`
- `isOffRow()` 判断 > 2048 字节使用 LOB 协议

**需要实现**:
1. `dameng/src/client.rs`: `execute_with_params()` 支持 CLOB/BLOB 参数
2. 小数据 (< 2048) 直接内联, 大数据使用 LOB 协议

**验收标准**:
- 能通过参数绑定插入 CLOB 数据
- 能通过参数绑定插入 BLOB 数据

---

### [ ] M4: FETCH 分页读取

**问题**: 大量结果集时，Rust 一次性读取所有行，Go 支持分批获取。

**Go 实现参考**:
- FETCH (msg_type=7) 消息: `row_count` 字段
- `BUF_PREFETCH` 属性控制预取行数

**需要实现**:
1. `dameng/src/client.rs`: `fetch_more(stmt_id: u32, row_count: u16)` 方法
2. `dameng/src/row.rs`: `ResultSet` 支持 `has_more()` 和 `fetch_more()`

**验收标准**:
- 百万行查询能分批读取，不 OOM

---

## LOW 优先级

### [ ] L1: 更精细的 DmValueType 枚举

**问题**: 现有 `DmValueType` 将 NUMERIC/DECIMAL、DATETIME/TIMESTAMP 等合并，不够精确。

**需要实现**:
1. 添加 `REAL`, `BOOLEAN`, `NUMERIC`, `VARCHAR2`, `DATETIME`, `DATETIME2`, `TIME_TZ`, `DATETIME_TZ`, `INTERVAL_DT`, `INTERVAL_YM`, `RAW` 枚举值
2. 更新 `from_type_code()`, `type_code()`, `type_name()`
3. 更新 `encode_value()`/`decode_value()`

**验收标准**:
- 各类型独立可区分
- 不影响现有 API

---

### [ ] L2: 输出参数 / RETURNING 支持

**问题**: Go 支持 ParameterDirection::Output/InputOutput，Rust 不支持。

**需要实现**:
1. `BindParam` 支持 `Output` 和 `InputOutput` 方向
2. 执行后从响应中读取输出参数值

**验收标准**:
- 存储过程输出参数能正确读取

---

### [ ] L3: SSL/TLS 支持

**问题**: Go 支持 `sslEncrypt` 字段，Rust 不支持加密连接。

**需要实现**:
1. 添加 `tokio-native-tls` 或 `rustls` 依赖
2. 连接时可选升级为 TLS

**验收标准**:
- 能通过 SSL 连接 DM 服务器

---

### [ ] L4: 错误码细化

**问题**: Go 有详细的错误码，Rust 错误分类较粗。

**需要实现**:
1. `dameng/src/error.rs`: 添加更细粒度的错误变体
   - `InvalidIsolation`
   - `LobFreed`
   - `InvalidDateFormat`
   - `ServerBusy`
   - 等

**验收标准**:
- 能区分不同类型的数据库错误

---

### [ ] L5: INTERVAL 参数绑定

**问题**: Go 支持 INTERVAL 类型参数绑定，Rust 不支持。

**需要实现**:
1. `BindParam` 支持 INTERVAL 类型
2. 编码 INTERVAL 值 (年-月或日-时-分-秒格式)

**验收标准**:
- 能通过参数绑定 INTERVAL 数据

---

## 进度追踪

| ID | 描述 | 优先级 | 状态 | 完成日期 |
|----|------|--------|------|----------|
| H1 | LOB 读取实现 | HIGH | 🟡 基础设施完成 (LobLocator + 检测), LOBREAD 待逆向 | |
| H2 | GB18030 编码转换 | HIGH | 🔴 未开始 | |
| M1 | 事务隔离级别 | MEDIUM | 🔴 未开始 | |
| M2 | 连接参数扩展 | MEDIUM | 🔴 未开始 | |
| M3 | CLOB/BLOB 参数绑定 | MEDIUM | 🔴 未开始 | |
| M4 | FETCH 分页读取 | MEDIUM | 🔴 未开始 | |
| L1 | 精细 DmValueType | LOW | 🔴 未开始 | |
| L2 | 输出参数支持 | LOW | 🔴 未开始 | |
| L3 | SSL/TLS 支持 | LOW | 🔴 未开始 | |
| L4 | 错误码细化 | LOW | 🔴 未开始 | |
| L5 | INTERVAL 参数绑定 | LOW | 🔴 未开始 | |

**图例**: 🔴 未开始 | 🟡 进行中 | 🟢 已完成 | ⚫ 已取消
