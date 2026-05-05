//! Sync client for connecting to Dameng database.

use std::io::{Read, Write};
use std::net::TcpStream;

use bytes::BytesMut;
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;

use crate::error::{Error, Result};
use crate::row::{Column, Row};

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
    state: State,
    host: String,
    port: u16,
    handle: u16,
    challenge: Vec<u8>,
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
        let msg = StartupMessage::new(&[]);
        let payload = msg.encode_payload();
        let frame_data = crate::build_message(msg_type::STARTUP, 0, &payload);
        self.write_all(&frame_data)?;
        Ok(())
    }

    /// Read the server's startup response.
    fn read_startup_response(&mut self) -> Result<StartupResponse> {
        let (frame, payload) = self.read_message()?;
        if frame.msg_type != msg_type::STARTUP_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected STARTUP_RESPONSE got msg_type={}",
                frame.msg_type
            )));
        }
        StartupResponse::from_bytes(&payload)
            .map_err(|e| Error::Protocol(e))
    }

    /// Send login credentials to the server.
    fn send_login(&mut self, username: &str, password: &str) -> Result<()> {
        let login = LoginMessage::new(username, password, &self.host);
        let payload = login.encode_payload(&self.challenge);
        let frame_data = crate::build_message(msg_type::LOGIN, 0, &payload);
        self.write_all(&frame_data)?;
        Ok(())
    }

    /// Read the login response.
    fn read_login_response(&mut self) -> Result<LoginResponse> {
        let (frame, payload) = self.read_message()?;
        if frame.msg_type != msg_type::LOGIN_RESPONSE {
            return Err(Error::ConnectionFailed(format!(
                "expected LOGIN_RESPONSE got msg_type={}",
                frame.msg_type
            )));
        }
        LoginResponse::from_bytes(&payload)
            .map_err(|e| Error::Protocol(e))
    }

    /// Execute a SQL statement without parameters.
    pub fn execute(&mut self, sql: &str) -> Result<Vec<Row>> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }

        // Send READY first
        let ready = ReadyMessage::new();
        let ready_payload = ready.encode_payload();
        self.write_all(&crate::build_message(msg_type::READY, self.handle, &ready_payload))?;
        self.read_message()?; // consume ACK

        // Send EXEC
        let exec = ExecMessage::new(sql, 0);
        let exec_payload = exec.encode_payload();
        self.handle += 1;
        self.write_all(&crate::build_message(msg_type::EXEC, self.handle, &exec_payload))?;
        self.read_exec_response()
    }

    /// Read an EXEC_RESPONSE and parse into Rows.
    fn read_exec_response(&mut self) -> Result<Vec<Row>> {
        let (frame, payload) = self.read_message()?;
        if frame.msg_type == msg_type::ACK {
            // Simple ACK - no result rows
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

    /// Send a READY keepalive and read the ACK.
    pub fn ready(&mut self) -> Result<()> {
        if !matches!(self.state, State::Ready) {
            return Err(Error::NotConnected);
        }
        let ready = ReadyMessage::new();
        let payload = ready.encode_payload();
        self.write_all(&crate::build_message(msg_type::READY, self.handle, &payload))?;
        let (frame, _) = self.read_message()?;
        if frame.msg_type != msg_type::ACK {
            return Err(Error::ConnectionFailed(format!(
                "expected ACK for READY got msg_type={}",
                frame.msg_type
            )));
        }
        Ok(())
    }

    /// Commit the current transaction.
    pub fn commit(&mut self) -> Result<()> {
        let commit = CommitMessage;
        let payload = commit.encode_payload();
        self.write_all(&crate::build_message(msg_type::COMMIT, self.handle, &payload))?;
        let (frame, _payload) = self.read_message()?;
        if frame.msg_type != msg_type::ACK {
            return Err(Error::ConnectionFailed(format!(
                "expected ACK for COMMIT got msg_type={}",
                frame.msg_type
            )));
        }
        Ok(())
    }

    /// Rollback the current transaction.
    pub fn rollback(&mut self) -> Result<()> {
        let rollback = RollbackMessage;
        let payload = rollback.encode_payload();
        self.write_all(&crate::build_message(msg_type::ROLLBACK, self.handle, &payload))?;
        let (frame, _) = self.read_message()?;
        if frame.msg_type != msg_type::ACK {
            return Err(Error::ConnectionFailed(format!(
                "expected ACK for ROLLBACK got msg_type={}",
                frame.msg_type
            )));
        }
        Ok(())
    }

    /// Read a complete message (frame + payload) from the stream.
    fn read_message(&mut self) -> Result<(Frame, Vec<u8>)> {
        let stream = self.stream.as_mut().ok_or(Error::NotConnected)?;
        let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + 4096);

        // Read frame header
        loop {
            if buf.len() >= FRAME_HEADER_SIZE {
                break;
            }
            let mut tmp = vec![0u8; 1024];
            let n = stream.read(&mut tmp)?;
            if n == 0 {
                return Err(Error::ConnectionFailed("connection closed".to_string()));
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        let frame = Frame::parse(&mut buf)?;

        // Read payload
        while buf.len() < FRAME_HEADER_SIZE + frame.payload_len as usize {
            let mut tmp = vec![0u8; 1024];
            let n = stream.read(&mut tmp)?;
            if n == 0 {
                return Err(Error::ConnectionFailed(
                    "connection closed during payload read".to_string(),
                ));
            }
            buf.extend_from_slice(&tmp[..n]);
        }

        let payload =
            buf[FRAME_HEADER_SIZE..FRAME_HEADER_SIZE + frame.payload_len as usize].to_vec();

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
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        self.state = State::Closed;
    }
}

/// Build a complete message (frame + payload).
pub fn build_message(msg_type: u16, handle: u16, payload: &[u8]) -> Vec<u8> {
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
