//! Sync client for connecting to Dameng database.

use std::io::{Read, Write};
use std::net::TcpStream;

use bytes::BytesMut;
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;

use crate::error::{Error, Result};
use crate::row::ResultSet;
use dameng_protocol::{Column, Row};

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
    pub handle: i32,
    /// Server challenge for encryption.
    pub challenge: Vec<u8>,
    /// Auto-commit mode.
    pub auto_commit: bool,
    /// Server encoding (1=UTF-8, 2=GB18030).
    pub encoding: u8,
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
            encoding: 1,
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
        if frame.response_code < 0 {
            return Err(Error::ConnectionFailed(format!(
                "allocate statement failed: code={}",
                frame.response_code
            )));
        }
        StatementAllocateMessage::parse_response(&resp_payload)
            .map_err(|e| Error::Protocol(e))
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
        self.write_all(&build_message(EXEC, stmt_id as i32, &exec_payload))?;
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
    pub fn execute_with_params(
        &mut self,
        stmt_id: u32,
        sql: &str,
        params: &[BindParam],
    ) -> Result<ResultSet> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let is_select = sql.trim_start().to_uppercase().starts_with("SELECT");

        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode())?;
        self.read_message()?;

        if params.is_empty() {
            let exec = ExecMessage::new(sql, 0);
            let exec_payload = exec.encode_payload();
            self.write_all(&build_message(
                EXEC,
                if stmt_id > 0 { stmt_id as i32 } else { 0 },
                &exec_payload,
            ))?;
            return self.read_exec_response();
        }

        let bind = BindExec2Message::new(self.auto_commit, is_select, params.to_vec());
        let bind_payload = bind.encode_payload();
        self.write_all(&build_message(BIND, stmt_id as i32, &bind_payload))?;
        self.read_exec_response()
    }

    /// Execute a SQL statement and return the number of affected rows.
    /// Use for DML: INSERT, UPDATE, DELETE, CREATE, DROP, COMMIT, ROLLBACK.
    /// When auto_commit is true (default), a COMMIT is sent after each statement.
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
            let resp = ExecResponse::from_bytes(&payload)?;
            return Ok(ResultSet { columns: resp.columns, rows: resp.rows });
        }
        if frame.msg_type == EXEC_RESPONSE || frame.msg_type == 160 {
            let resp = ExecResponse::from_bytes(&payload)?;
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
pub fn build_message(msg_type: u8, handle: i32, payload: &[u8]) -> Vec<u8> {
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
