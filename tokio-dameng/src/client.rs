//! Async client for connecting to Dameng database using tokio.

use bytes::BytesMut;
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::{Error, Result};
use crate::row::{Column, Row};

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
    handle: u16,
    challenge: Vec<u8>,
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
        let msg = StartupMessage::new(&[]);
        let payload = msg.encode_payload();
        let frame_data = crate::build_message(msg_type::STARTUP, 0, &payload);
        self.write_all(&frame_data).await?;
        Ok(())
    }

    async fn read_startup_response(&mut self) -> Result<StartupResponse> {
        let (frame, payload) = self.read_message().await?;
        if frame.msg_type != msg_type::STARTUP_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected STARTUP_RESPONSE got msg_type={}",
                frame.msg_type
            )));
        }
        StartupResponse::from_bytes(&payload).map_err(|e| Error::Protocol(e))
    }

    async fn send_login(&mut self, username: &str, password: &str) -> Result<()> {
        let login = LoginMessage::new(username, password, &self.host);
        let payload = login.encode_payload(&self.challenge);
        let frame_data = crate::build_message(msg_type::LOGIN, 0, &payload);
        self.write_all(&frame_data).await?;
        Ok(())
    }

    async fn read_login_response(&mut self) -> Result<LoginResponse> {
        let (frame, payload) = self.read_message().await?;
        if frame.msg_type != msg_type::LOGIN_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected LOGIN_RESPONSE got msg_type={}",
                frame.msg_type
            )));
        }
        LoginResponse::from_bytes(&payload).map_err(|e| Error::Protocol(e))
    }

    /// Execute a SQL statement and return result rows.
    pub async fn execute(&mut self, sql: &str) -> Result<Vec<Row>> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        let ready = ReadyMessage::new();
        let ready_payload = ready.encode_payload();
        self.write_all(&crate::build_message(msg_type::READY, self.handle, &ready_payload)).await?;
        self.read_message().await?;

        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.handle += 1;
        self.write_all(&crate::build_message(msg_type::EXEC, self.handle, &exec_payload)).await?;
        self.read_exec_response().await
    }

    async fn read_exec_response(&mut self) -> Result<Vec<Row>> {
        let (frame, payload) = self.read_message().await?;
        if frame.msg_type == msg_type::ACK {
            return Ok(vec![]);
        }
        if frame.msg_type == msg_type::EXEC_RESPONSE {
            let resp = ExecResponse::from_bytes(&payload)?;
            let mut rows = Vec::new();
            for row_data in resp.rows {
                let columns: Vec<Column> = resp
                    .columns
                    .iter()
                    .map(|c| Column {
                        name: c.name.clone(),
                        type_code: c.type_code,
                        type_name: c.type_name.clone(),
                        precision: c.precision,
                        scale: c.scale,
                        nullable: c.nullable,
                    })
                    .collect();
                rows.push(Row {
                    columns,
                    values: row_data.values,
                });
            }
            Ok(rows)
        } else {
            Err(Error::ConnectionFailed(format!(
                "unexpected response msg_type={}",
                frame.msg_type
            )))
        }
    }

    /// Commit the current transaction.
    pub async fn commit(&mut self) -> Result<()> {
        let commit = CommitMessage;
        let payload = commit.encode_payload();
        self.write_all(&crate::build_message(msg_type::COMMIT, self.handle, &payload)).await?;
        self.read_message().await?;
        Ok(())
    }

    /// Rollback the current transaction.
    pub async fn rollback(&mut self) -> Result<()> {
        let rollback = RollbackMessage;
        let payload = rollback.encode_payload();
        self.write_all(&crate::build_message(msg_type::ROLLBACK, self.handle, &payload)).await?;
        self.read_message().await?;
        Ok(())
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

        while buf.len() < FRAME_HEADER_SIZE + frame.payload_len as usize {
            let mut tmp = vec![0u8; 1024];
            let n = stream.read(&mut tmp).await?;
            if n == 0 {
                return Err(Error::ConnectionFailed("connection closed during payload".to_string()));
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        let payload = buf[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + frame.payload_len as usize].to_vec();
        Ok((frame, payload))
    }

    async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        let stream = self.stream.as_mut().ok_or(Error::NotConnected)?;
        stream.write_all(data).await?;
        Ok(())
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.state = State::Closed;
    }
}

pub(crate) fn build_message(msg_type: u16, handle: u16, payload: &[u8]) -> Vec<u8> {
    let frame = Frame::new(msg_type, handle, payload.len() as u16);
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
        assert_eq!(frame.payload_len, 10);
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
