use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use bytes::{BufMut, BytesMut};
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;

fn read_exact(stream: &mut TcpStream, buf: &mut Vec<u8>, len: usize) -> Result<(), String> {
    let mut total = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while total < len {
        let mut tmp = vec![0u8; 4096.min(len - total)];
        match stream.read(&mut tmp) {
            Ok(0) => return Err("Connection closed".to_string()),
            Ok(n) => { buf.extend_from_slice(&tmp[..n]); total += n; }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == Some(35) => {
                if std::time::Instant::now() > deadline { return Err("Timeout".to_string()); }
                std::thread::sleep(Duration::from_millis(10)); continue;
            }
            Err(e) => return Err(format!("Read error: {}", e)),
        }
    }
    Ok(())
}

fn read_message(stream: &mut TcpStream) -> Result<(Frame, Vec<u8>), String> {
    let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + 4096);
    loop {
        if buf.len() >= FRAME_HEADER_SIZE { break; }
        let mut tmp = vec![0u8; 1024];
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {}", e))?;
        if n == 0 { return Err("Connection closed".to_string()); }
        buf.extend_from_slice(&tmp[..n]);
    }
    let frame = Frame::parse(&mut buf).map_err(|e| format!("parse frame: {:?}", e))?;
    let body_len = frame.body_len.max(0) as usize;
    while buf.len() < body_len {
        let mut tmp = vec![0u8; 1024];
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {}", e))?;
        if n == 0 { return Err("Connection closed".to_string()); }
        buf.extend_from_slice(&tmp[..n]);
    }
    let payload = buf[..body_len].to_vec();
    Ok((frame, payload))
}

fn hex_dump(data: &[u8], prefix: &str) {
    println!("{} ({} bytes):", prefix, data.len());
    for i in (0..data.len()).step_by(16) {
        let chunk = &data[i..(i + 16).min(data.len())];
        let hex = chunk.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
        let ascii = chunk.iter().map(|b| if *b >= 32 && *b < 127 { *b as char } else { '.' }).collect::<String>();
        println!("  {:04x}: {:<48}  {}", i, hex, ascii);
    }
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").expect("Failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    // === STARTUP ===
    println!("=== STARTUP ===");
    let startup = StartupMessage::new();
    let startup_payload = startup.encode_payload();
    println!("Startup payload from library:");
    hex_dump(&startup_payload, "  ");

    let startup_frame = Frame::new(STARTUP, 0, startup_payload.len() as i32);
    let mut startup_msg = startup_frame.encode();
    startup_msg.put_slice(&startup_payload);
    stream.write_all(&startup_msg).unwrap();
    println!("Sent STARTUP");

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("Response: msg_type={} body_len={}", frame.msg_type, frame.body_len);
    hex_dump(&body, "Response body");

    let startup_resp = StartupResponse::from_bytes(&body, frame.response_code).unwrap();
    println!("Server version: {}", startup_resp.server_version);
    println!("Challenge len: {}", startup_resp.challenge.len());
    if !startup_resp.challenge.is_empty() {
        println!("Challenge (first 16): {}", startup_resp.challenge[..16.min(startup_resp.challenge.len())].iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));
    }

    // === LOGIN ===
    println!("\n=== LOGIN ===");
    let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
    let login_payload = login.encode_payload(&startup_resp.challenge);
    println!("Login payload from library:");
    hex_dump(&login_payload, "  ");

    let login_frame = Frame::new(LOGIN, 0, login_payload.len() as i32);
    let mut login_msg = login_frame.encode();
    login_msg.put_slice(&login_payload);
    stream.write_all(&login_msg).unwrap();
    println!("Sent LOGIN");

    match read_message(&mut stream) {
        Ok((frame, body)) => {
            println!("Response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
            hex_dump(&body, "Response body");
            if frame.msg_type == LOGIN_RESPONSE {
                let login_resp = LoginResponse::from_bytes(&body).unwrap();
                println!("Server name: {}", login_resp.server_name);
                println!("Username: {}", login_resp.username);
                println!("DB name: {}", login_resp.db_name);
            }
        }
        Err(e) => {
            println!("Login response error: {}", e);
        }
    }

    println!("\n=== Done ===");
}
