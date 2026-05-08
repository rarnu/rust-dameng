use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use bytes::{BufMut, BytesMut};
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;

fn hex_dump(data: &[u8], prefix: &str) {
    println!("{} ({} bytes):", prefix, data.len());
    for i in (0..data.len()).step_by(16) {
        let chunk = &data[i..(i + 16).min(data.len())];
        let hex = chunk.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
        let ascii = chunk.iter().map(|b| if *b >= 32 && *b < 127 { *b as char } else { '.' }).collect::<String>();
        println!("  {:04x}: {:<48}  {}", i, hex, ascii);
    }
}

fn read_message(stream: &mut TcpStream) -> Result<(Frame, Vec<u8>), String> {
    let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + 8192);
    loop {
        if buf.len() >= FRAME_HEADER_SIZE { break; }
        let mut tmp = vec![0u8; 4096];
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {}", e))?;
        if n == 0 { return Err("Connection closed".to_string()); }
        buf.extend_from_slice(&tmp[..n]);
    }
    let frame = Frame::parse(&mut buf).map_err(|e| format!("parse frame: {:?}", e))?;
    let body_len = frame.body_len.max(0) as usize;
    while buf.len() < body_len {
        let mut tmp = vec![0u8; 4096];
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {}", e))?;
        if n == 0 { return Err("connection closed".to_string()); }
        buf.extend_from_slice(&tmp[..n]);
    }
    let payload = buf[..body_len].to_vec();
    Ok((frame, payload))
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").expect("Failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    println!("Connected to DM server");

    // === STARTUP ===
    let startup = StartupMessage::new();
    let startup_payload = startup.encode_payload();
    let startup_frame = Frame::new(STARTUP, 0, startup_payload.len() as i32);
    let mut startup_msg = startup_frame.encode();
    startup_msg.put_slice(&startup_payload);
    stream.write_all(&startup_msg).unwrap();
    println!("Sent STARTUP ({} bytes)", startup_msg.len());

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("STARTUP response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
    hex_dump(&body, "  body");

    let startup_resp = StartupResponse::from_bytes(&body, frame.response_code).unwrap();
    println!("  version={} challenge_len={}", startup_resp.server_version, startup_resp.challenge.len());

    // === LOGIN ===
    let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
    let login_payload = login.encode_payload(&startup_resp.challenge);
    let login_frame = Frame::new(LOGIN, 0, login_payload.len() as i32);
    let mut login_msg = login_frame.encode();
    login_msg.put_slice(&login_payload);
    stream.write_all(&login_msg).unwrap();
    println!("\nSent LOGIN ({} bytes)", login_msg.len());

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("LOGIN response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);

    let login_resp = LoginResponse::from_bytes(&body).unwrap();
    println!("  server={} user={} db={}", login_resp.server_name, login_resp.username, login_resp.db_name);

    // === EXEC: SELECT 1 FROM DUAL ===
    // Send EXACTLY what the library client sends:
    // ExecMessage::new("SELECT 1 FROM DUAL", 0).encode_payload()
    // Then build_message(EXEC, 0, &exec_payload)
    let sql = "SELECT 1 FROM DUAL";
    let exec = ExecMessage::new(sql, 0);
    let exec_payload = exec.encode_payload();
    println!("\n=== EXEC ===");
    hex_dump(&exec_payload, "  exec payload");

    let exec_frame = Frame::new(EXEC, 0, exec_payload.len() as i32);
    let mut exec_msg = exec_frame.encode();
    exec_msg.put_slice(&exec_payload);
    hex_dump(&exec_msg, "  full exec frame");

    stream.write_all(&exec_msg).unwrap();
    println!("Sent EXEC ({} bytes total)", exec_msg.len());

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("\nEXEC response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
    hex_dump(&body, "  body");

    if frame.msg_type == ACK {
        println!("  -> Got ACK, no result set");
    } else if frame.msg_type == EXEC_RESPONSE {
        println!("  -> Got EXEC_RESPONSE, parsing...");
        match ExecResponse::from_bytes(&body) {
            Ok(resp) => {
                println!("  col_count={} row_count={}", resp.col_count, resp.row_count);
                for (i, col) in resp.columns.iter().enumerate() {
                    println!("  col[{}]: name='{}' type_code={} type_name='{}'", i, col.name, col.type_code, col.type_name);
                }
                for (i, row) in resp.rows.iter().enumerate() {
                    println!("  row[{}]: id={} values={:?}", i, row.row_id, row.values);
                }
            }
            Err(e) => println!("  Parse error: {:?}", e),
        }
    } else {
        println!("  -> Unexpected msg_type={}", frame.msg_type);
        // Try to decode error message from body
        if body.len() > 12 {
            let msg_len = u32::from_le_bytes([body[12], body.get(13).copied().unwrap_or(0), body.get(14).copied().unwrap_or(0), body.get(15).copied().unwrap_or(0)]) as usize;
            if msg_len > 0 && body.len() >= 16 + msg_len {
                println!("  Error message: {}", String::from_utf8_lossy(&body[16..16+msg_len]));
            }
        }
    }

    // === COMMIT ===
    let commit_frame = Frame::new(COMMIT, 0, 0);
    let commit_msg = commit_frame.encode();
    stream.write_all(&commit_msg).unwrap();
    println!("\n=== COMMIT ===");
    println!("Sent COMMIT ({} bytes)", commit_msg.len());

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("COMMIT response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
    hex_dump(&body, "  body");
    // Try to decode message
    if body.len() > 12 {
        let msg_len = u32::from_le_bytes([body[12], body.get(13).copied().unwrap_or(0), body.get(14).copied().unwrap_or(0), body.get(15).copied().unwrap_or(0)]) as usize;
        if msg_len > 0 && body.len() >= 16 + msg_len {
            println!("  Message: {}", String::from_utf8_lossy(&body[16..16+msg_len]));
        }
    }
}
