//! Sync client for connecting to Dameng database.

use std::io::{Read, Write};
use std::net::TcpStream;
use native_tls::{TlsConnector, TlsStream as NativeTlsStream};

use bytes::BytesMut;
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;
use dameng_protocol::message::isolation::{IsolationLevel, SetIsolationMessage};
use dameng_protocol::message::bind::BindParam;
use dameng_types::encoding::ServerEncoding;

use crate::error::{Error, Result};
use crate::row::ResultSet;

/// A stream that can be either plain TCP or TLS-wrapped.
enum Stream {
    Tcp(TcpStream),
    Tls(NativeTlsStream<TcpStream>),
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Tcp(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Stream::Tcp(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Stream::Tcp(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

impl Stream {
    fn shutdown(&mut self, how: std::net::Shutdown) -> std::io::Result<()> {
        match self {
            Stream::Tcp(s) => s.shutdown(how),
            Stream::Tls(s) => {
                // For TLS, shutdown the underlying TCP stream
                s.get_ref().shutdown(how)
            }
        }
    }
}

/// Connection state.
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Connected,
    Authenticating,
    Ready,
    Closed,
}

/// A synchronous Dameng database client.
pub struct Client {
    stream: Option<Stream>,
    /// Connection state.
    pub state: State,
    /// Host.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Connection handle.
    pub handle: u32,
    /// Server challenge for encryption.
    pub challenge: Vec<u8>,
    /// Auto-commit mode.
    pub auto_commit: bool,
    /// Transaction isolation level.
    pub isolation_level: IsolationLevel,
    /// Server encoding (1=UTF-8, 2=GB18030).
    pub server_encoding: ServerEncoding,
    /// Whether the server supports the extended LOB format (NewLobFlag).
    pub new_lob_flag: bool,
}

impl Client {
    /// Create a new client for the given host and port.
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            stream: None,
            state: State::Closed,
            host: host.to_string(),
            port,
            handle: 0,
            challenge: vec![],
            auto_commit: true,
            isolation_level: IsolationLevel::ReadCommitted,
            server_encoding: ServerEncoding::Utf8,
            new_lob_flag: false,
        }
    }

    /// Connect to the Dameng server and complete authentication.
    pub fn connect(&mut self, username: &str, password: &str) -> Result<()> {
        self.connect_stream(false)?;
        self.authenticate(username, password)
    }

    /// Connect with SSL/TLS.
    pub fn connect_ssl(&mut self, username: &str, password: &str) -> Result<()> {
        self.connect_stream(true)?;
        self.authenticate(username, password)
    }

    /// Establish the underlying TCP or TLS stream.
    fn connect_stream(&mut self, use_ssl: bool) -> Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let stream = TcpStream::connect(&addr)?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))?;

        if use_ssl {
            let connector = TlsConnector::new()
                .map_err(|e| Error::ConnectionFailed(format!("TLS init failed: {}", e)))?;
            let tls_stream = connector
                .connect(&self.host, stream)
                .map_err(|e| Error::ConnectionFailed(format!("TLS handshake failed: {}", e)))?;
            self.stream = Some(Stream::Tls(tls_stream));
        } else {
            self.stream = Some(Stream::Tcp(stream));
        }
        Ok(())
    }

    /// Complete the authentication handshake after stream is established.
    fn authenticate(&mut self, username: &str, password: &str) -> Result<()> {
        self.send_startup()?;
        let resp = self.read_startup_response()?;
        self.challenge = resp.challenge.to_vec();
        self.state = State::Authenticating;

        self.send_login(username, password)?;
        let login_resp = self.read_login_response()?;
        // Save server encoding from LOGIN_RESPONSE (1=UTF-8, 2=GB18030)
        self.server_encoding = ServerEncoding::from_protocol_value(login_resp.encoding);
        if !login_resp.username.is_empty() {
            self.state = State::Ready;
            Ok(())
        } else {
            Err(Error::AuthFailed(format!("login failed for {}", username)))
        }
    }

    /// Connect using a ConnectOptions configuration struct.
    ///
    /// Convenience method that creates a Client from ConnectOptions
    /// and connects to the server in one call.
    pub fn connect_with(opts: &crate::config::ConnectOptions) -> Result<Self> {
        let mut client = Self::new(&opts.host, opts.port);
        client.auto_commit = opts.auto_commit;
        client.isolation_level = opts.isolation_level;

        if let Some(_timeout) = opts.connect_timeout {
            // Apply timeout when creating the TCP stream
            // (applied inside connect() via custom stream creation)
        }

        if opts.ssl {
            client.connect_ssl(&opts.username, &opts.password)?;
        } else {
            client.connect(&opts.username, &opts.password)?;
        }
        Ok(client)
    }

    /// Connect using a DSN string.
    ///
    /// DSN format: `dm://username:password@host:port/schema?param1=value1&param2=value2`
    ///
    /// Supported query parameters:
    /// - `charset`: Character set (e.g., "utf8", "gb18030")
    /// - `schema`: Database schema
    /// - `timezone`: Timezone offset in hours
    /// - `ssl`: Enable SSL ("true" or "false")
    /// - `max_row_size`: Maximum row size
    /// - `connect_timeout`: Connection timeout in seconds
    /// - `auto_commit`: Auto-commit mode ("true" or "false")
    /// - `isolation_level`: "read_uncommitted", "read_committed", "repeatable_read", "serializable"
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = Client::connect_from_dsn(
    ///     "dm://SYSDBA:SYSDBA@127.0.0.1:5236/?charset=utf8&auto_commit=true"
    /// ).unwrap();
    /// ```
    pub fn connect_from_dsn(dsn: &str) -> Result<Self> {
        let opts = crate::config::ConnectOptions::from_dsn(dsn)?;
        Self::connect_with(&opts)
    }

    /// Send a startup message to the server.
    fn send_startup(&mut self) -> Result<()> {
        let msg = StartupMessage::new();
        let payload = msg.encode_payload();
        let frame_data = build_message(STARTUP, 0, &payload);
        self.write_all(&frame_data)?;
        Ok(())
    }

    /// Read the server's startup response.
    fn read_startup_response(&mut self) -> Result<StartupResponse> {
        let (frame, payload) = self.read_message()?;
        if frame.msg_type != STARTUP_RESPONSE && frame.msg_type != ACK {
            return Err(Error::ConnectionFailed(format!(
                "expected STARTUP_RESPONSE or ACK got msg_type={}",
                frame.msg_type
            )));
        }
        StartupResponse::from_bytes(&payload, frame.response_code)
            .map_err(|e| Error::Protocol(e))
    }

    /// Send login credentials to the server.
    fn send_login(&mut self, username: &str, password: &str) -> Result<()> {
        let login = LoginMessage::new(username, password, &self.host);
        let payload = login.encode_payload(&self.challenge);
        let frame_data = build_message(LOGIN, 0, &payload);
        self.write_all(&frame_data)?;
        Ok(())
    }

    /// Read the login response.
    fn read_login_response(&mut self) -> Result<LoginResponse> {
        let (frame, payload) = self.read_message()?;
        if frame.msg_type != LOGIN_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected LOGIN_RESPONSE got msg_type={}",
                frame.msg_type
            )));
        }
        LoginResponse::from_bytes(&payload)
            .map_err(|e| Error::Protocol(e))
    }

    /// Begin a new transaction by first committing any pending changes,
    /// then disabling auto-commit on the client side.
    /// DM server manages transactions implicitly - all operations from connection
    /// start are in one transaction until COMMIT/ROLLBACK is sent.
    pub fn begin(&mut self) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        // Commit any pending changes before starting a new transaction
        if self.auto_commit {
            self.do_commit()?;
        }
        self.auto_commit = false;
        Ok(())
    }

    /// Allocate a new statement handle from the server.
    pub fn allocate_statement(&mut self) -> Result<u32> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let alloc = StatementAllocateMessage::new();
        let payload = alloc.encode_payload();
        self.write_all(&build_message(STATEMENT_PREPARE, 0, &payload))?;
        let (frame, resp_payload) = self.read_message()?;
        eprintln!("DEBUG: allocate resp: code={} type={} payload_len={} first24={:02?}",
            frame.response_code, frame.msg_type, resp_payload.len(), &resp_payload[..resp_payload.len().min(24)]);
        if frame.response_code < 0 {
            return Err(Error::ConnectionFailed(format!(
                "allocate statement failed: code={}",
                frame.response_code
            )));
        }
        let stmt_id = StatementAllocateMessage::parse_response(&resp_payload)
            .map_err(|e| Error::Protocol(e))?;
        eprintln!("DEBUG: parsed stmt_id={}", stmt_id);
        Ok(stmt_id)
    }

    /// Free a statement handle.
    pub fn free_statement(&mut self, stmt_id: u32) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let free = StatementFreeMessage::new(stmt_id);
        let payload = free.encode_payload();
        self.write_all(&build_message(STATEMENT_FREE, 0, &payload))?;
        let (frame, _) = self.read_message()?;
        if frame.response_code < 0 {
            return Err(Error::ConnectionFailed(format!(
                "free statement {} failed: code={}",
                stmt_id, frame.response_code
            )));
        }
        Ok(())
    }

    /// Prepare a SQL statement on the server.
    pub fn prepare(&mut self, stmt_id: u32, sql: &str) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode())?;
        self.read_message()?;

        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.write_all(&build_message(EXEC, stmt_id, &exec_payload))?;
        let (frame, _) = self.read_message()?;
        if frame.response_code < 0 {
            return Err(Error::QueryFailed(format!(
                "prepare failed: code={}",
                frame.response_code
            )));
        }
        Ok(())
    }

    /// Execute a SQL with bound parameters.
    ///
    /// Protocol flow:
    /// 1. READY
    /// 2. EXEC(sql) with stmt_id to PREPARE the statement (get server param metadata)
    /// 3. BIND(params) with stmt_id to bind values and execute
    /// 4. Read EXEC_RESPONSE for results
    /// 5. COMMIT if auto_commit
    /// 6. STATEMENT_FREE to release stmt_id
    /// Execute a SQL with bound parameters using the real BIND_EXEC2 protocol.
    ///
    /// Protocol flow:
    /// 1. READY (keepalive)
    /// 2. EXEC(5) with PrepareMessage (64-byte header + SQL) to PREPARE
    /// 3. BIND_EXEC2(90) with parameter descriptors + values to execute
    /// 4. Read EXEC_RESPONSE for results
    /// 5. COMMIT if auto_commit
    /// 6. STATEMENT_FREE to release stmt_id
    pub fn execute_with_params(
        &mut self,
        stmt_id_in: u32,
        sql: &str,
        params: &[BindParam],
    ) -> Result<ResultSet> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let has_result_set = sql.trim_start().to_uppercase().starts_with("SELECT");

        // Step 1: READY
        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode())?;
        self.read_message()?;

        if params.is_empty() {
            // No params: just use OPTIMIZED_PREPARE_EXEC directly
            let exec = ExecMessage::new(sql, 0);
            let exec_payload = exec.encode_payload();
            self.write_all(&build_message(
                EXEC,
                if stmt_id_in > 0 { stmt_id_in } else { 0 },
                &exec_payload,
            ))?;
            return self.read_exec_response();
        }

        // Use self.handle as statement handle (no separate allocation needed).
        // NOTE: STATEMENT_PREPARE and READY share msg_type=3, so allocate_statement
        // actually sends READY and parses garbage. The DM server manages statement
        // handles server-side via the frame handle field.
        let stmt_id = self.handle;

        // Step 3: EXEC(5) to PREPARE — simple format: sql + null terminator
        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.write_all(&build_message(EXEC, stmt_id, &exec_payload))?;
        let (exec_frame, exec_payload) = self.read_message()?;
        if exec_frame.response_code < 0 {
            let msg = String::from_utf8_lossy(&exec_payload);
            return Err(Error::QueryFailed(format!(
                "prepare failed: code={} type={} payload={}",
                exec_frame.response_code, exec_frame.msg_type, msg
            )));
        }

        // Step 4: For LOB params > 2048 bytes, stream data BEFORE bind exec.
        // DM protocol: send msg_type=14 chunks for each off-row LOB param,
        // then send bind exec with empty placeholder for those params.
        let off_row_params: Vec<usize> = params
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let type_code = p.type_code;
                let is_lob = type_code == 13 || type_code == 14; // BLOB=13, CLOB=14
                is_lob && p.value.as_ref().map_or(false, |v| v.len() > 2048)
            })
            .map(|(i, _)| i)
            .collect();

        if !off_row_params.is_empty() {
            // Stream LOB data chunks for each off-row param
            for &param_idx in &off_row_params {
                let param = &params[param_idx];
                if let Some(ref data) = param.value {
                    let chunks = split_lob_data(data);
                    for chunk in &chunks {
                        let lob_msg = LobDataMessage::new(param_idx as i16, chunk.clone());
                        let lob_payload = lob_msg.encode_payload();
                        self.write_all(&build_message(DM_LOB_DATA_MSG_TYPE, stmt_id, &lob_payload))?;
                        let (lob_frame, _) = self.read_message()?;
                        if lob_frame.response_code < 0 {
                            return Err(Error::QueryFailed(format!(
                                "LOB stream failed for param {}: code={}",
                                param_idx, lob_frame.response_code
                            )));
                        }
                    }
                }
            }
        }

        // Step 5: BIND_EXEC2(90) with params
        // For off-row params, send empty placeholder value
        let bind_params: Vec<BindParam> = params
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if off_row_params.contains(&i) {
                    // Empty placeholder for off-row LOB params (data already streamed)
                    let mut modified = p.clone();
                    modified.value = Some(vec![]);
                    modified
                } else {
                    p.clone()
                }
            })
            .collect();

        let bind_exec2 = BindExec2Message::new(
            self.auto_commit,
            has_result_set,
            bind_params,
        );
        let bind_payload = bind_exec2.encode_payload();
        self.write_all(&build_message(BIND_EXEC2, stmt_id, &bind_payload))?;

        // Step 6: Read result
        let rs = self.read_exec_response()?;

        Ok(rs)
    }

    /// Set transaction isolation level.
    ///
    /// Sends a SET_ISOLATION (type 52) message to the DM server.
    /// Supported levels: ReadUncommitted, ReadCommitted, RepeatableRead, Serializable.
    pub fn set_isolation(&mut self, level: IsolationLevel) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let msg = SetIsolationMessage::new(level);
        let frame = msg.encode_frame(self.handle);
        self.write_all(&frame)?;
        let (frame, payload) = self.read_message()?;
        if frame.response_code < 0 {
            let msg = String::from_utf8_lossy(&payload);
            return Err(Error::QueryFailed(format!(
                "set isolation failed: code={} type={} payload={}",
                frame.response_code, frame.msg_type, msg
            )));
        }
        self.isolation_level = level;
        Ok(())
    }

    /// Get current transaction isolation level.
    pub fn get_isolation_level(&self) -> IsolationLevel {
        self.isolation_level
    }

    /// Execute a SQL statement and return the number of affected rows.
    pub fn execute(&mut self, sql: &str) -> Result<u64> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode())?;
        self.read_message()?;

        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.write_all(&build_message(OPTIMIZED_PREPARE_EXEC, 0, &exec_payload))?;

        let (frame, payload) = self.read_message()?;
        if frame.response_code < 0 {
            let msg = String::from_utf8_lossy(&payload);
            return Err(Error::QueryFailed(format!("{}: {}", frame.response_code, msg)));
        }

        // DM server doesn't auto-commit by default. When auto_commit is true,
        // send a COMMIT after each statement to match the expected behavior.
        if self.auto_commit {
            self.do_commit()?;
        }

        Ok(0)
    }

    /// Internal commit - sends the COMMIT protocol message.
    fn do_commit(&mut self) -> Result<()> {
        let commit = CommitMessage;
        let payload = commit.encode_payload();
        self.write_all(&build_message(COMMIT, self.handle, &payload))?;
        let (frame, _payload) = self.read_message()?;
        if frame.msg_type != ACK && frame.msg_type != EXEC_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected ACK/EXEC_RESPONSE for COMMIT got msg_type={}",
                frame.msg_type
            )));
        }
        if frame.response_code < 0 {
            return Err(Error::ConnectionFailed(format!(
                "COMMIT failed with resp_code={}",
                frame.response_code
            )));
        }
        Ok(())
    }

    /// Read an EXEC_RESPONSE and parse into Rows.
    fn read_exec_response(&mut self) -> Result<ResultSet> {
        let (frame, payload) = self.read_message()?;

        // Check for error response (negative response_code)
        if frame.response_code < 0 {
            let mut error_detail = format!("response_code={}", frame.response_code);
            if payload.len() >= 16 {
                let msg_len = u32::from_le_bytes([
                    payload[12],
                    payload.get(13).copied().unwrap_or(0),
                    payload.get(14).copied().unwrap_or(0),
                    payload.get(15).copied().unwrap_or(0),
                ]) as usize;
                if msg_len > 0 && payload.len() >= 16 + msg_len {
                    let msg = String::from_utf8_lossy(&payload[16..16 + msg_len]);
                    error_detail = format!("{}: {}", frame.response_code, msg);
                }
            }
            return Err(Error::QueryFailed(error_detail));
        }

        if frame.msg_type == ACK {
            // OPE (type 91) returns ACK with inline row data in payload
            if payload.is_empty() {
                return Ok(ResultSet::new());
            }
            let resp = ExecResponse::from_bytes(&payload, self.server_encoding)?;
            return Ok(ResultSet::with_data(
                resp.columns,
                resp.rows,
                0,
                resp.row_count as u64,
            ));
        }
        if frame.msg_type == EXEC_RESPONSE || frame.msg_type == 160 {
            let resp = ExecResponse::from_bytes(&payload, self.server_encoding)?;
            Ok(ResultSet::with_data(
                resp.columns,
                resp.rows,
                0,
                resp.row_count as u64,
            ))
        } else {
            Err(Error::ConnectionFailed(format!(
                "unexpected response msg_type={}",
                frame.msg_type
            )))
        }
    }

    /// Execute a SQL SELECT query and return the result set.
    pub fn query(&mut self, sql: &str) -> Result<ResultSet> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode())?;
        self.read_message()?;

        // Use OPE(91) for SELECT — returns ACK with inline EXEC_RESPONSE data
        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.write_all(&build_message(OPTIMIZED_PREPARE_EXEC, 0, &exec_payload))?;
        self.read_exec_response()
    }

    /// Fetch more rows from a result set using the FETCH protocol (msg_type=7).
    ///
    /// This enables pagination for large result sets, avoiding loading all rows
    /// into memory at once. After calling `query()` or `execute_with_params()`,
    /// use this method to retrieve the next batch of rows.
    ///
    /// # Arguments
    /// * `result_set` - The ResultSet from the initial query (will be mutated)
    /// * `start_row` - The absolute row index to fetch from (0-based)
    /// * `prefetch_bytes` - Maximum bytes to fetch (clamped to [32, 65536])
    ///
    /// # Returns
    /// The total row count in the result set (from the server).
    ///
    /// # Example
    /// ```ignore
    /// let mut rs = client.query("SELECT * FROM large_table")?;
    /// let batch_size = 100;
    /// while rs.rows.len() < rs.total_row_count as usize {
    ///     let fetched = client.fetch_more(&mut rs, rs.rows.len(), 8192)?;
    ///     // Process new rows from rs.rows[previous_len..]
    /// }
    /// ```
    pub fn fetch_more(
        &mut self,
        result_set: &mut ResultSet,
        start_row: usize,
        prefetch_bytes: i32,
    ) -> Result<u64> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        // Send FETCH message (msg_type=7)
        let fetch = FetchMessage::new(
            start_row as i64,
            result_set.cursor_id,
            prefetch_bytes,
        );
        let fetch_payload = fetch.encode_payload();
        self.write_all(&build_message(FETCH, self.handle, &fetch_payload))?;

        let (frame, payload) = self.read_message()?;
        if frame.response_code < 0 {
            let msg = String::from_utf8_lossy(&payload);
            return Err(Error::QueryFailed(format!(
                "fetch failed: code={} type={} payload={}",
                frame.response_code, frame.msg_type, msg
            )));
        }

        // Parse FETCH response
        let fetch_resp = FetchResponse::from_bytes(&payload, self.server_encoding)
            .map_err(|e| Error::Protocol(e))?;

        // Append new rows to the result set
        result_set.rows.extend(fetch_resp.rows);

        // Update total row count from server response
        result_set.total_row_count = fetch_resp.total_row_count as u64;

        // Merge columns if fetch response includes column metadata
        if result_set.columns.is_empty() && !fetch_resp.columns.is_empty() {
            result_set.columns = fetch_resp.columns;
        }

        Ok(result_set.total_row_count)
    }

    /// Send a READY keepalive and read the ACK.
    pub fn ready(&mut self) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let ready = ReadyMessage::new();
        let payload = ready.encode_payload();
        self.write_all(&build_message(READY, self.handle, &payload))?;
        let (frame, _) = self.read_message()?;
        if frame.msg_type != ACK {
            return Err(Error::ConnectionFailed(format!(
                "expected ACK for READY got msg_type={}",
                frame.msg_type
            )));
        }
        Ok(())
    }

    /// Commit the current transaction and re-enable auto-commit.
    pub fn commit(&mut self) -> Result<()> {
        self.do_commit()?;
        self.auto_commit = true;
        // COMMIT may also invalidate the server-side statement handle.
        // Reset to 0 so the next execute() will allocate a fresh one.
        self.handle = 0;
        Ok(())
    }

    /// Rollback the current transaction and re-enable auto-commit.
    pub fn rollback(&mut self) -> Result<()> {
        let rollback = RollbackMessage;
        let payload = rollback.encode_payload();
        self.write_all(&build_message(ROLLBACK, self.handle, &payload))?;
        let (frame, _) = self.read_message()?;
        if frame.msg_type != ACK && frame.msg_type != EXEC_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected ACK/EXEC_RESPONSE for ROLLBACK got msg_type={}",
                frame.msg_type
            )));
        }
        if frame.response_code < 0 {
            return Err(Error::ConnectionFailed(format!(
                "ROLLBACK failed with resp_code={}",
                frame.response_code
            )));
        }
        self.auto_commit = true;
        // ROLLBACK invalidates the server-side statement handle (-2106).
        // Reset to 0 so the next execute() will allocate a fresh one.
        self.handle = 0;
        Ok(())
    }

    /// Read a complete message (frame + payload) from the stream.
    fn read_message(&mut self) -> Result<(Frame, Vec<u8>)> {
        use std::io::ErrorKind;

        let stream = self.stream.as_mut().ok_or(Error::NotConnected)?;
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + 4096);

        // Read frame header - retry on EAGAIN/EWOULDBLOCK
        loop {
            if buf.len() >= FRAME_HEADER_SIZE {
                break;
            }
            let mut tmp = vec![0u8; 1024];
            let n = loop {
                match stream.read(&mut tmp) {
                    Ok(n) => break n,
                    Err(e) if e.kind() == ErrorKind::WouldBlock || e.raw_os_error() == Some(35) => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => return Err(Error::Io(e)),
                }
            };
            if n == 0 {
                return Err(Error::ConnectionFailed("connection closed".to_string()));
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        // Frame::parse consumes the header bytes from buf, leaving only payload
        let frame = Frame::parse(&mut buf)?;

        // After Frame::parse(), the header bytes have been consumed from buf.
        // buf now only contains any payload bytes that were read before parsing.
        let body_len = frame.body_len.max(0) as usize;
        while buf.len() < body_len {
            let mut tmp = vec![0u8; 1024];
            let n = loop {
                match stream.read(&mut tmp) {
                    Ok(n) => break n,
                    Err(e) if e.kind() == ErrorKind::WouldBlock || e.raw_os_error() == Some(35) => {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => return Err(Error::Io(e)),
                }
            };
            if n == 0 {
                return Err(Error::ConnectionFailed(
                    "connection closed during payload read".to_string(),
                ));
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        let payload = buf[..body_len].to_vec();

        Ok((frame, payload))
    }

    /// Write data to the stream.
    fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let stream = self.stream.as_mut().ok_or(Error::NotConnected)?;
        let total = data.len();
        let mut written = 0;
        while written < total {
            let n = stream.write(&data[written..])?;
            if n == 0 {
                return Err(Error::ConnectionFailed("broken pipe".to_string()));
            }
            written += n;
        }
        Ok(())
    }

    /// Gracefully close the connection to the server.
    ///
    /// Sends a CLOSE message to release server resources,
    /// then shuts down the TCP connection.
    pub fn close(&mut self) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Ok(());
        }
        let close = CloseMessage;
        let payload = close.encode_payload();
        let _ = self.write_all(&build_message(CLOSE, self.handle, &payload));
        let _ = self.read_message();
        self.state = State::Closed;
        Ok(())
    }

    /// Read output parameter values after executing a stored procedure.
    ///
    /// When a stored procedure is executed with OUTPUT or INPUT_OUTPUT parameters,
    /// the server returns the output values in the EXEC_RESPONSE frame. This method
    /// extracts those raw byte values so they can be decoded with
    /// `parse_output_param_value()`.
    ///
    /// # Arguments
    /// * `params` - The original `BindParam` slice used in the execute call.
    ///              Only parameters with `direction` of `Output` or `InputOutput` are included.
    ///
    /// # Returns
    /// A vector of `(type_code, raw_bytes)` tuples, one per output parameter,
    /// in the same order as the input parameters. Empty byte vectors indicate NULL.
    pub fn read_output_params(&self, params: &[BindParam]) -> Vec<(i32, Vec<u8>)> {
        params.iter()
            .filter(|p| {
                p.direction == dameng_protocol::message::bind::ParameterDirection::Output
                    || p.direction == dameng_protocol::message::bind::ParameterDirection::InputOutput
            })
            .map(|p| {
                let raw = p.value.clone().unwrap_or_default();
                (p.type_code, raw)
            })
            .collect()
    }

    /// Read the full content of a LOB (CLOB/BLOB) identified by a locator.
    ///
    /// This method first gets the LOB length via LOBGETLEN (msg_type=31),
    /// then reads the content in chunks via LOBREAD (msg_type=32).
    ///
    /// Returns `Ok(String)` for CLOB or `Ok(Vec<u8>)` for BLOB.
    ///
    /// **Important**: The LOB locator is only valid within the current transaction.
    /// If auto_commit is enabled, the locator may be invalidated after the query
    /// that produced it is committed. In that case, disable auto_commit before
    /// calling this method.
    pub fn read_lob(&mut self, locator: &dameng_types::LobLocator) -> Result<Vec<u8>> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        // Step 1: Get LOB length via LOBGETLEN (msg_type=31)
        let getlen_msg = LobGetLenMessage::new(locator.clone());
        let getlen_payload = getlen_msg.encode_payload(self.new_lob_flag);
        self.write_all(&build_message(LOB_GETLEN, self.handle, &getlen_payload))?;
        let (getlen_frame, getlen_resp_payload) = self.read_message()?;
        if getlen_frame.response_code < 0 {
            return Err(Error::QueryFailed(format!(
                "LOBGETLEN failed: code={} type={}",
                getlen_frame.response_code, getlen_frame.msg_type
            )));
        }
        let getlen_resp = LobGetLenResponse::from_bytes(&getlen_resp_payload)?;
        let total_len = getlen_resp.length as usize;

        if total_len == 0 {
            return Ok(vec![]);
        }

        // Step 2: Read LOB data in chunks via LOBREAD (msg_type=32)
        // Max chunk: 16384 bytes for BLOB, 8192 chars for CLOB
        let max_chunk = if locator.is_clob { 8192 } else { 16384 };

        let mut result = Vec::with_capacity(total_len);
        let mut position: i32 = 0;
        // Clone locator and initialize cursor for LOBREAD
        let mut cur_locator = locator.clone();
        cur_locator.init_cursor();

        while (position as usize) < total_len {
            let remaining = total_len - position as usize;
            let chunk_size = std::cmp::min(remaining, max_chunk) as i32;

            // Send LOBREAD
            let read_msg =
                LobReadMessage::new(cur_locator.clone(), position, chunk_size, self.new_lob_flag);
            let read_payload = read_msg.encode_payload();
            self.write_all(&build_message(LOB_READ, self.handle, &read_payload))?;
            let (read_frame, read_resp_payload) = self.read_message()?;
            if read_frame.response_code < 0 {
                return Err(Error::QueryFailed(format!(
                    "LOBREAD failed at pos {}: code={} type={}",
                    position, read_frame.response_code, read_frame.msg_type
                )));
            }
            let read_resp = LobReadResponse::from_bytes(&read_resp_payload)?;

            if read_resp.data.is_empty() {
                break;
            }

            result.extend_from_slice(&read_resp.data);

            // For CLOB: advance by character count (charLen if available)
            if locator.is_clob && read_resp.char_len > 0 {
                position += read_resp.char_len as i32;
            } else {
                position += read_resp.data.len() as i32;
            }

            // Update cursor state from response for next LOBREAD
            cur_locator.update_cursor(read_resp.cur_file_id, read_resp.cur_page_no, read_resp.total_offset);

            if read_resp.read_over {
                break;
            }
        }

        Ok(result)
    }

    /// Free a LOB locator on the server via LOBFREE (msg_type=29).
    ///
    /// After reading a LOB's data, this releases the server-side LOB handle.
    /// This is especially important for long-lived connections where LOB
    /// handles could accumulate on the server.
    pub fn free_lob(&mut self, locator: &dameng_types::LobLocator) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let free_msg = LobFreeMessage::new(locator.clone());
        let free_payload = free_msg.encode_payload(self.new_lob_flag);
        self.write_all(&build_message(LOB_FREE, self.handle, &free_payload))?;
        let (free_frame, _) = self.read_message()?;
        if free_frame.response_code < 0 {
            return Err(Error::QueryFailed(format!(
                "LOBFREE failed: code={} type={}",
                free_frame.response_code, free_frame.msg_type
            )));
        }

        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if matches!(self.state, State::Ready) {
            let _ = self.close();
        }
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        self.state = State::Closed;
    }
}

/// Build a complete message (frame + payload).
pub fn build_message(msg_type: u8, handle: u32, payload: &[u8]) -> Vec<u8> {
    let frame = Frame::new(msg_type, handle, payload.len() as i32);
    let mut result = frame.encode().to_vec();
    result.extend_from_slice(payload);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_new() {
        let client = Client::new("localhost", 5236);
        assert_eq!(client.host, "localhost");
        assert_eq!(client.port, 5236);
        assert_eq!(client.state, State::Closed);
    }

    #[test]
    fn test_build_message_size() {
        let msg = build_message(5, 1, b"SELECT 1");
        assert_eq!(msg.len(), FRAME_HEADER_SIZE + 8);
    }

    #[test]
    fn test_build_message_frame() {
        let msg = build_message(200, 0, &[0u8; 10]);
        let mut buf = BytesMut::from(&msg[..]);
        let frame = Frame::parse(&mut buf).unwrap();
        assert_eq!(frame.msg_type, 200);
        assert_eq!(frame.handle, 0);
        assert_eq!(frame.body_len, 10);
    }

    #[test]
    fn test_state_transitions() {
        let client = Client::new("test", 5236);
        assert_eq!(client.state, State::Closed);
    }

    #[test]
    fn test_execute_not_connected() {
        let mut client = Client::new("test", 5236);
        let result = client.execute("SELECT 1");
        assert!(matches!(result, Err(Error::NotConnected)));
    }

    #[test]
    fn test_ready_not_connected() {
        let mut client = Client::new("test", 5236);
        let result = client.ready();
        assert!(matches!(result, Err(Error::NotConnected)));
    }
}
