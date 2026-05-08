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
    println!("Connected to DM server\n");

    // === STARTUP ===
    let startup = StartupMessage::new();
    let startup_payload = startup.encode_payload();
    let startup_frame = Frame::new(STARTUP, 0, startup_payload.len() as i32);
    let mut startup_msg = startup_frame.encode();
    startup_msg.put_slice(&startup_payload);
    stream.write_all(&startup_msg).unwrap();
    println!("Sent STARTUP ({} bytes)", startup_msg.len());

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("STARTUP response: msg_type={} body_len={} resp_code={}\n", frame.msg_type, frame.body_len, frame.response_code);

    let startup_resp = StartupResponse::from_bytes(&body, frame.response_code).unwrap();
    println!("  version={} challenge_len={}\n", startup_resp.server_version, startup_resp.challenge.len());

    // === LOGIN ===
    let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
    let login_payload = login.encode_payload(&startup_resp.challenge);
    let login_frame = Frame::new(LOGIN, 0, login_payload.len() as i32);
    let mut login_msg = login_frame.encode();
    login_msg.put_slice(&login_payload);
    stream.write_all(&login_msg).unwrap();
    println!("Sent LOGIN ({} bytes)", login_msg.len());

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("LOGIN response: msg_type={} body_len={} resp_code={}\n", frame.msg_type, frame.body_len, frame.response_code);

    let login_resp = LoginResponse::from_bytes(&body).unwrap();
    println!("  server={} user={}\n", login_resp.server_name, login_resp.username);

    // === READY (empty payload, exactly like debug_all) ===
    println!("=== READY ===");
    let ready_frame = Frame::new(READY, 0, 0);
    let ready_msg = ready_frame.encode();
    hex_dump(&ready_msg, "  READY frame");
    stream.write_all(&ready_msg).unwrap();
    println!("Sent READY ({} bytes)\n", ready_msg.len());

    let (frame, _body) = read_message(&mut stream).unwrap();
    println!("READY response: msg_type={}\n", frame.msg_type);

    // === EXEC: SELECT 1 FROM DUAL ===
    // EXACTLY like debug_all: SQL + null terminator, handle=0
    println!("=== EXEC ===");
    let sql = b"SELECT 1 FROM DUAL";
    let mut exec_payload = Vec::new();
    exec_payload.extend_from_slice(sql);
    exec_payload.push(0);
    hex_dump(&exec_payload, "  exec payload");

    let exec_frame = Frame::new(EXEC, 0, exec_payload.len() as i32);
    let mut exec_msg = exec_frame.encode();
    exec_msg.put_slice(&exec_payload);
    hex_dump(&exec_msg, "  full exec frame");

    stream.write_all(&exec_msg).unwrap();
    println!("\nSent EXEC ({} bytes total)", exec_msg.len());

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("\nEXEC response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
    hex_dump(&body, "  body");

    if frame.msg_type == EXEC_RESPONSE {
        println!("\n-> Got EXEC_RESPONSE!");
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
        println!("\n-> NOT EXEC_RESPONSE, got msg_type={}", frame.msg_type);
        if body.len() > 12 {
            let msg_len = u32::from_le_bytes([body[12], body.get(13).copied().unwrap_or(0), body.get(14).copied().unwrap_or(0), body.get(15).copied().unwrap_or(0)]) as usize;
            if msg_len > 0 && body.len() >= 16 + msg_len {
                println!("  Error message: {}", String::from_utf8_lossy(&body[16..16+msg_len]));
            }
        }
    }
}
