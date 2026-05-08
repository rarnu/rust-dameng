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

    // STARTUP
    let startup = StartupMessage::new();
    let startup_payload = startup.encode_payload();
    let startup_frame = Frame::new(STARTUP, 0, startup_payload.len() as i32);
    let mut startup_msg = startup_frame.encode();
    startup_msg.put_slice(&startup_payload);
    stream.write_all(&startup_msg).unwrap();
    let (_frame, body) = read_message(&mut stream).unwrap();
    let startup_resp = StartupResponse::from_bytes(&body, 0).unwrap();

    // LOGIN
    let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
    let login_payload = login.encode_payload(&startup_resp.challenge);
    let login_frame = Frame::new(LOGIN, 0, login_payload.len() as i32);
    let mut login_msg = login_frame.encode();
    login_msg.put_slice(&login_payload);
    stream.write_all(&login_msg).unwrap();
    read_message(&mut stream).unwrap();

    // EXEC: SELECT VERSION FROM V$VERSION
    println!("=== EXEC: SELECT VERSION FROM V$VERSION ===");
    let sql = b"SELECT VERSION FROM V$VERSION";
    let mut exec_payload = Vec::new();
    exec_payload.extend_from_slice(sql);
    exec_payload.push(0);

    // Send READY first (empty payload)
    let ready_frame = Frame::new(READY, 0, 0);
    stream.write_all(&ready_frame.encode()).unwrap();
    println!("Sent READY");
    let (frame, _) = read_message(&mut stream).unwrap();
    println!("  READY response: msg_type={}\n", frame.msg_type);

    let exec_frame = Frame::new(EXEC, 0, exec_payload.len() as i32);
    let mut exec_msg = exec_frame.encode();
    exec_msg.put_slice(&exec_payload);
    stream.write_all(&exec_msg).unwrap();
    println!("Sent EXEC ({} bytes total)\n", exec_msg.len());

    // Read ALL responses
    let mut msg_num = 0;
    loop {
        let (frame, body) = match read_message(&mut stream) {
            Ok(r) => r,
            Err(e) => { println!("\n  Error reading: {}", e); break; }
        };
        msg_num += 1;
        println!("--- Message {} ---", msg_num);
        println!("  msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
        // Only dump first 256 bytes
        hex_dump(&body[..body.len().min(256)], "  body (first 256)");
        if body.len() > 256 {
            println!("  ... ({} bytes total)", body.len());
            hex_dump(&body[body.len()-64..], "  body (last 64)");
        }

        // Stop after 5 messages
        if msg_num >= 5 { break; }
    }
}
