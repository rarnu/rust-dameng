//! Async client for connecting to Dameng database using tokio.

use bytes::BytesMut;
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{Error, Result};
use crate::row::ResultSet;
use dameng_protocol::{Column, Row};

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Connected,
    Authenticating,
    Ready,
    Closed,
}

pub struct Client {
    stream: Option<TcpStream>,
    state: State,
    host: String,
    port: u16,
    handle: i32,
    challenge: Vec<u8>,
    auto_commit: bool,
    encoding: u8,
}

impl Client {
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

    pub async fn connect(&mut self, username: &str, password: &str) -> Result<()> {
        let stream = TcpStream::connect(format!("{}:{}", self.host, self.port)).await?;
        stream.set_nodelay(true)?;
        self.stream = Some(stream);

        self.send_startup().await?;
        let resp = self.read_startup_response().await?;
        self.challenge = resp.challenge.to_vec();
        self.state = State::Authenticating;

        self.send_login(username, password).await?;
        let login_resp = self.read_login_response().await?;
        if !login_resp.username.is_empty() {
            self.state = State::Ready;
            log::info!("Connected to Dameng as {} on {}", login_resp.username, login_resp.server_name);
            Ok(())
        } else {
            Err(Error::AuthFailed(format!("login failed for {}", username)))
        }
    }

    async fn send_startup(&mut self) -> Result<()> {
        let msg = StartupMessage::new();
        let payload = msg.encode_payload();
        let frame_data = build_message(STARTUP, 0, &payload);
        self.write_all(&frame_data).await?;
        Ok(())
    }

    async fn read_startup_response(&mut self) -> Result<StartupResponse> {
        let (frame, payload) = self.read_message().await?;
        if frame.msg_type != STARTUP_RESPONSE && frame.msg_type != ACK {
            return Err(Error::ConnectionFailed(format!(
                "expected STARTUP_RESPONSE or ACK got msg_type={}",
                frame.msg_type
            )));
        }
        StartupResponse::from_bytes(&payload, frame.response_code).map_err(|e| Error::Protocol(e))
    }

    async fn send_login(&mut self, username: &str, password: &str) -> Result<()> {
        let login = LoginMessage::new(username, password, &self.host);
        let payload = login.encode_payload(&self.challenge);
        let frame_data = build_message(LOGIN, 0, &payload);
        self.write_all(&frame_data).await?;
        Ok(())
    }

    async fn read_login_response(&mut self) -> Result<LoginResponse> {
        let (frame, payload) = self.read_message().await?;
        if frame.msg_type != LOGIN_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected LOGIN_RESPONSE got msg_type={}",
                frame.msg_type
            )));
        }
        LoginResponse::from_bytes(&payload).map_err(|e| Error::Protocol(e))
    }

    /// Begin a new transaction by first committing any pending changes,
    /// then disabling auto-commit on the client side.
    pub async fn begin(&mut self) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        // Commit any pending changes before starting a new transaction
        if self.auto_commit {
            self.do_commit().await?;
        }
        self.auto_commit = false;
        Ok(())
    }

    /// Allocate a new statement handle from the server.
    pub async fn allocate_statement(&mut self) -> Result<u32> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let alloc = StatementAllocateMessage::new();
        let payload = alloc.encode_payload();
        self.write_all(&build_message(STATEMENT_PREPARE, 0, &payload)).await?;
        let (frame, resp_payload) = self.read_message().await?;
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
    pub async fn free_statement(&mut self, stmt_id: u32) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let free = StatementFreeMessage::new(stmt_id);
        let payload = free.encode_payload();
        self.write_all(&build_message(STATEMENT_FREE, 0, &payload)).await?;
        let (frame, _) = self.read_message().await?;
        if frame.response_code < 0 {
            return Err(Error::ConnectionFailed(format!(
                "free statement {} failed: code={}",
                stmt_id, frame.response_code
            )));
        }
        Ok(())
    }

    /// Prepare a SQL statement on the server.
    pub async fn prepare(&mut self, stmt_id: u32, sql: &str) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode()).await?;
        self.read_message().await?;

        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.write_all(&build_message(EXEC, stmt_id as i32, &exec_payload)).await?;
        let (frame, _) = self.read_message().await?;
        if frame.response_code < 0 {
            return Err(Error::QueryFailed(format!(
                "prepare failed: code={}",
                frame.response_code
            )));
        }
        Ok(())
    }

    /// Execute a SQL with bound parameters.
    pub async fn execute_with_params(
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
        self.write_all(&ready_frame.encode()).await?;
        self.read_message().await?;

        if params.is_empty() {
            let exec = ExecMessage::new(sql, 0);
            let exec_payload = exec.encode_payload();
            self.write_all(&build_message(
                EXEC,
                if stmt_id > 0 { stmt_id as i32 } else { 0 },
                &exec_payload,
            )).await?;
            return self.read_exec_response().await;
        }

        let bind = BindExec2Message::new(self.auto_commit, is_select, params.to_vec());
        let bind_payload = bind.encode_payload();
        self.write_all(&build_message(BIND, stmt_id as i32, &bind_payload)).await?;
        self.read_exec_response().await
    }

    /// Execute a SQL statement and return the number of affected rows.
    /// Use for DML: INSERT, UPDATE, DELETE, CREATE, DROP, COMMIT, ROLLBACK.
    /// When auto_commit is true (default), a COMMIT is sent after each statement.
    pub async fn execute(&mut self, sql: &str) -> Result<u64> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode()).await?;
        self.read_message().await?;

        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.write_all(&build_message(OPTIMIZED_PREPARE_EXEC, 0, &exec_payload)).await?;

        let (frame, payload) = self.read_message().await?;
        if frame.response_code < 0 {
            let msg = String::from_utf8_lossy(&payload);
            return Err(Error::QueryFailed(format!("{}: {}", frame.response_code, msg)));
        }

        // DM server doesn't auto-commit by default. When auto_commit is true,
        // send a COMMIT after each statement to match the expected behavior.
        if self.auto_commit {
            self.do_commit().await?;
        }

        Ok(0)
    }

    /// Execute a SQL SELECT query and return the result set.
    pub async fn query(&mut self, sql: &str) -> Result<ResultSet> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let ready_frame = Frame::new(READY, 0, 0);
        self.write_all(&ready_frame.encode()).await?;
        self.read_message().await?;

        // Use OPE(91) for SELECT — returns ACK with inline EXEC_RESPONSE data
        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.write_all(&build_message(OPTIMIZED_PREPARE_EXEC, 0, &exec_payload)).await?;
        self.read_exec_response().await
    }

    /// Send a READY keepalive and read the ACK.
    pub async fn ready(&mut self) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let ready = ReadyMessage::new();
        let payload = ready.encode_payload();
        self.write_all(&build_message(READY, self.handle, &payload)).await?;
        let (frame, _) = self.read_message().await?;
        if frame.msg_type != ACK {
            return Err(Error::ConnectionFailed(format!(
                "expected ACK for READY got msg_type={}",
                frame.msg_type
            )));
        }
        Ok(())
    }

    /// Internal commit - sends the COMMIT protocol message.
    async fn do_commit(&mut self) -> Result<()> {
        let commit = CommitMessage;
        let payload = commit.encode_payload();
        self.write_all(&build_message(COMMIT, self.handle, &payload)).await?;
        let (frame, _payload) = self.read_message().await?;
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

    /// Commit the current transaction and re-enable auto-commit.
    pub async fn commit(&mut self) -> Result<()> {
        self.do_commit().await?;
        self.auto_commit = true;
        // COMMIT may also invalidate the server-side statement handle.
        // Reset to 0 so the next execute() will allocate a fresh one.
        self.handle = 0;
        Ok(())
    }

    /// Rollback the current transaction and re-enable auto-commit.
    pub async fn rollback(&mut self) -> Result<()> {
        let rollback = RollbackMessage;
        let payload = rollback.encode_payload();
        self.write_all(&build_message(ROLLBACK, self.handle, &payload)).await?;
        let (frame, _) = self.read_message().await?;
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

    async fn read_exec_response(&mut self) -> Result<ResultSet> {
        let (frame, payload) = self.read_message().await?;

        // Check for error response first
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
                    error_detail = format!(
                        "{}: {}",
                        frame.response_code,
                        String::from_utf8_lossy(&payload[16..16 + msg_len])
                    );
                }
            }
            return Err(Error::QueryFailed(error_detail));
        }

        // Helper to convert ExecResponse into ResultSet
        let parse_rows = |payload: &[u8]| -> Result<ResultSet> {
            let resp = ExecResponse::from_bytes(payload)?;
            let rows: Vec<Row> = resp.rows;
            Ok(ResultSet { columns: resp.columns, rows })
        };

        if frame.msg_type == ACK {
            // OPE (type 91) returns ACK with inline row data in payload
            if payload.is_empty() {
                return Ok(ResultSet::new());
            }
            return parse_rows(&payload);
        }
        if frame.msg_type == EXEC_RESPONSE || frame.msg_type == 160 {
            return parse_rows(&payload);
        }
        Err(Error::ConnectionFailed(format!(
            "unexpected response msg_type={}",
            frame.msg_type
        )))
    }

    async fn read_message(&mut self) -> Result<(Frame, Vec<u8>)> {
        let stream = self.stream.as_mut().ok_or(Error::NotConnected)?;
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + 4096);

        loop {
            if buf.len() >= FRAME_HEADER_SIZE {
                break;
            }
            let mut tmp = vec![0u8; 1024];
            let n = stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(Error::ConnectionFailed("connection closed".to_string()));
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        let frame = Frame::parse(&mut buf)?;

        let body_len = frame.body_len.max(0) as usize;
        while buf.len() < body_len {
            let mut tmp = vec![0u8; 1024];
            let n = stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(Error::ConnectionFailed("connection closed during payload".to_string()));
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        let payload = buf[..body_len].to_vec();
        Ok((frame, payload))
    }

    async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let stream = self.stream.as_mut().ok_or(Error::NotConnected)?;
        stream.write_all(data).await?;
        Ok(())
    }

    /// Gracefully close the connection to the server.
    ///
    /// Sends a CLOSE message to release server resources,
    /// then shuts down the TCP connection.
    pub async fn close(&mut self) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Ok(());
        }
        let close = CloseMessage;
        let payload = close.encode_payload();
        let _ = self.write_all(&build_message(CLOSE, self.handle, &payload)).await;
        let _ = self.read_message().await;
        self.state = State::Closed;
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.state = State::Closed;
    }
}

pub(crate) fn build_message(msg_type: u8, handle: i32, payload: &[u8]) -> Vec<u8> {
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

    #[tokio::test]
    async fn test_execute_not_connected() {
        let mut client = Client::new("test", 5236);
        let result = client.execute("SELECT 1").await;
        assert!(matches!(result, Err(Error::NotConnected)));
    }

    #[tokio::test]
    async fn test_commit_not_connected() {
        let mut client = Client::new("test", 5236);
        let result = client.commit().await;
        assert!(matches!(result, Err(Error::NotConnected)));
    }

    #[tokio::test]
    async fn test_rollback_not_connected() {
        let mut client = Client::new("test", 5236);
        let result = client.rollback().await;
        assert!(matches!(result, Err(Error::NotConnected)));
    }
}
