//! Sync client for connecting to Dameng database.

use std::io::{Read, Write};
use std::net::TcpStream;

use bytes::BytesMut;
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;
use dameng_protocol::message::isolation::{IsolationLevel, SetIsolationMessage};
use dameng_protocol::message::bind::BindParam;
use dameng_types::encoding::ServerEncoding;

use crate::error::{Error, Result};
use crate::row::ResultSet;

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
    stream: Option<TcpStream>,
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
        let stream = TcpStream::connect(format!("{}:{}", self.host, self.port))?;
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))?;
        self.stream = Some(stream);

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

        // Step 4: BIND_EXEC2(90) with params
        let bind_exec2 = BindExec2Message::new(
            self.auto_commit,
            has_result_set,
            params.to_vec(),
        );
        let bind_payload = bind_exec2.encode_payload();
        self.write_all(&build_message(BIND_EXEC2, stmt_id, &bind_payload))?;

        // Step 5: Read result
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
            return Ok(ResultSet { columns: resp.columns, rows: resp.rows });
        }
        if frame.msg_type == EXEC_RESPONSE || frame.msg_type == 160 {
            let resp = ExecResponse::from_bytes(&payload, self.server_encoding)?;
            Ok(ResultSet { columns: resp.columns, rows: resp.rows })
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
        let mut position: usize = 0;
        // Copy locator to track cursor position across reads
        let mut cur_locator = locator.clone();

        while position < total_len {
            let remaining = total_len - position;
            let chunk_size = std::cmp::min(remaining, max_chunk) as i32;

            // Send READY before LOBREAD (DM protocol requirement)
            let ready_frame = Frame::new(READY, 0, 0);
            self.write_all(&ready_frame.encode())?;
            self.read_message()?;

            let read_msg =
                LobReadMessage::new(cur_locator.clone(), position as i32, chunk_size, self.new_lob_flag);
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
            position += read_resp.data.len();

            // Update cursor for next read
            cur_locator = cur_locator.clone();

            if read_resp.read_over {
                break;
            }
        }

        Ok(result)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if matches!(self.state, State::Ready) {
            let _ = self.close();
        }
        if let Some(stream) = self.stream.take() {
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
