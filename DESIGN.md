# DESIGN.md - Rust Dameng Database Driver

## Architecture

Multi-crate workspace following rust-postgres structure:

```
rust-dameng/
  Cargo.toml              # Workspace definition
  dameng-protocol/        # Wire protocol implementation (like postgres-protocol)
  dameng-types/           # Type definitions and conversions (like postgres-types)
  dameng/                 # Sync connection (like postgres)
  tokio-dameng/           # Async connection with tokio (like tokio-postgres)
  examples/               # Usage examples
  scripts/                # Helper scripts
```

## Phase 1: Workspace + dameng-protocol

### 1.1 Frame Format

Every DM message has a 64-byte header:
```
Offset  Size  Field
0       4     Version (LE u32, always 0)
4       2     MsgType (LE u16)
6       2     Handle (LE u16, statement/connection handle)
8       4     Reserved (LE u32)
12      4     Reserved (LE u32)
16      2     PayloadLen (LE u16)
18      16    Reserved (zeros)
34      2     Reserved (LE u16)
36      4     Checksum (LE u32)
40      24    Reserved (zeros)
```

### 1.2 Message Types

Client->Server:
- 200: STARTUP (initial connection handshake)
- 1: LOGIN (send credentials)
- 3: READY (send ready/keepalive)
- 5: PREPARE/EXEC (prepare statement or execute)
- 13: BIND (bind parameters and execute)
- 8: COMMIT
- 7: ROLLBACK
- 20: CLOSE (close statement)
- 21: FETCH (fetch more rows)

Server->Client:
- 228: STARTUP_RESPONSE (server hello, sends encryption key)
- 163: LOGIN_RESPONSE (session info, server version)
- 187: READY/ACK (success response)
- 0: EXEC_RESPONSE (statement result with column metadata + rows)

### 1.3 Startup Payload (type 200, 82 bytes)
```
0x00  u16  flags1
0x02  u16  flags2
0x04  u32  reserved
0x08  u32  reserved
0x0C  u16  reserved
0x0E  [128B] encrypted random key (from server challenge)
```

### 1.4 Startup Response Payload (type 228, 112 bytes)
```
0x00  u16  flags1
0x02  u32  reserved
0x06  u16  client_major_version
0x08  u16  client_minor_version
0x0A  u16  reserved
0x0C  u8   encoding (UTF-8=1, GB18030=2)
0x0D  [48B] server challenge (random bytes for encryption)
0x3D  u32  reserved
0x41  [12B] server version string
0x4D  u32  reserved
0x51  [64B] encryption public key
0x91  u16  reserved
0x93  u16  reserved
0x95  u16  reserved
0x97  u32  reserved
```

### 1.5 Login Payload (type 1, 59 bytes)
```
0x00  u32  isolation_level
0x04  u32  reserved
0x08  u16  language_id (1=EN, 2=CN)
0x0A  u16  reserved
0x0C  u32  reserved
0x10  u16  client_codepage
0x12  u16  reserved
0x14  u16  reserved
0x16  u16  reserved
0x18  [20B] reserved
0x28  u32  username_len
0x2C  [128B] username (null-padded, encrypted)
0x9C  u32  password_len
0xA0  [128B] password (null-padded, encrypted)
```

### 1.6 Login Response (type 163, 159 bytes)
```
0x00  u16  flags
0x02  u32  session_id
0x06  u32  reserved
0x0A  u16  reserved
0x0C  u16  reserved
0x0E  u8   encoding (1=UTF-8, 2=GB18030)
0x0F  u8   reserved
0x10  u16  reserved
0x12  u32  reserved
0x16  u8   server_status
0x17  [7B] reserved
0x20  u32  server_name_len
0x24  [128B] server_name
0x9C  u32  username_len
0xA0  [128B] username
0x10C u32  ip_len
0x110  [48B] client_ip
0x13C u32  datetime_len
0x140  [48B] login_datetime
```

### 1.7 Prepare/Execute Payload (type 5)
```
0x00  u8   is_prepared (1=yes, 0=direct exec)
0x01  u8   reserved
0x02  u16  reserved
0x04  u16  param_count
0x06  u32  reserved
0x0A  u16  reserved
0x0C  u32  reserved
0x10  u32  reserved
0x14  u32  reserved
0x18  [SQL string] (null-terminated, server encoding)
```

### 1.8 Bind Payload (type 13)
```
0x00  u8   fetch_flag (1=fetch after bind)
0x01  u8   reserved
0x02  u16  reserved
0x04  u16  param_count
0x06  u16  reserved
0x08  u32  reserved
0x0C  u32  reserved
0x10  u32  reserved
0x14  u32  reserved
Then for each param:
  u16  param_type_name_len
  [type name, e.g., "INT", "VARCHAR"]
  u32  param_type_code (4=INT, etc.)
  u32  precision
  u32  scale
  u16  value_len
  [value bytes]
```

### 1.9 Execute Response (type 0)
```
0x00  u16  flags
0x02  u16  reserved
0x04  u32  rows_affected
0x08  u16  param_count (for prepared stmt)
0x0A  u16  reserved
0x0C  u32  stmt_handle
0x10  u16  col_count
0x12  u16  reserved
0x14  [column metadata...]
Then rows...
```

### 1.10 Column Metadata (per column)
```
u16  col_name_len
[col_name bytes]
u32  col_type_code (4=INT, 3=VARCHAR, etc.)
u16  reserved
u16  precision
u16  scale
u16  nullable
u16  display_size
```

### 1.11 Row Data (per row)
```
u16  row_total_size (includes this field)
i64  row_id
Then for each column:
  u16  value_len
  [value bytes, if len > 0]
```

### 1.12 DM Type Codes

```
1    BIT/BOOLEAN
2    TINYINT
3    VARCHAR/CHAR
4    INT
5    BIGINT
6    SMALLINT
7    FLOAT
8    DOUBLE
9    DECIMAL/NUMERIC
10   DATE
11   TIME
12   TIMESTAMP
13   BLOB
14   CLOB
15   INTERVAL
```

## Phase 2: dameng-types

Rust types mapping:
- DmText -> String
- DmInt2 -> i16
- DmInt4 -> i32
- DmInt8 -> i64
- DmFloat4 -> f32
- DmFloat8 -> f64
- DmDate -> chrono::Date
- DmTimestamp -> chrono::DateTime
- DmBytea -> Vec<u8>
- DmBoolean -> bool
- DmDecimal -> rust_decimal::Decimal

## Phase 3: dameng (sync)

Simple sync connection using std::net::TcpStream.

## Phase 4: tokio-dameng

Async connection using tokio::net::TcpStream, tokio::io::AsyncRead/AsyncWrite.

## Testing

- Use the DM instance at 127.0.0.1:5236/SYSDBA
- Create test tables in tests/ setup
- Drop test tables in tests/ teardown
- Test all CRUD operations, transactions, parameter binding
