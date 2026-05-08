use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use dameng::Client;

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
    // Connect directly to see what the library sends
    let mut stream = TcpStream::connect("127.0.0.1:5236").expect("Failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

    // Build startup message using library
    let startup = dameng_protocol::message::StartupMessage::new();
    let startup_payload = startup.encode_payload();
    println!("=== Library StartupMessage ===");
    hex_dump(&startup_payload, "  payload");

    let startup_frame = dameng_protocol::frame::Frame::new(
        dameng_protocol::message::STARTUP,
        0,
        startup_payload.len() as i32,
    );
    let mut startup_msg = startup_frame.encode();
    startup_msg.extend_from_slice(&startup_payload);
    hex_dump(&startup_msg, "  full frame");

    stream.write_all(&startup_msg).unwrap();
    println!("Sent STARTUP");

    // Read response
    let mut header = vec![];
    read_exact(&mut stream, &mut header, 64).unwrap();
    let msg_type = header[4];
    let body_len = i32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
    let mut body = vec![];
    if body_len > 0 { read_exact(&mut stream, &mut body, body_len).unwrap(); }
    println!("Response: msg_type={} body_len={}", msg_type, body_len);
    hex_dump(&body, "  response body");

    // Parse startup response
    let startup_resp = dameng_protocol::message::StartupResponse::from_bytes(&body, 0).unwrap();
    println!("Server version: {}", startup_resp.server_version);
    println!("Challenge len: {}", startup_resp.challenge.len());
    if startup_resp.challenge.len() > 0 {
        println!("Challenge (first 16): {}", startup_resp.challenge[..16.min(startup_resp.challenge.len())].iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));
    }

    // Build login message using library
    let login = dameng_protocol::message::LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
    let login_payload = login.encode_payload(&startup_resp.challenge);
    println!("\n=== Library LoginMessage ===");
    hex_dump(&login_payload, "  payload");

    let login_frame = dameng_protocol::frame::Frame::new(
        dameng_protocol::message::LOGIN,
        0,
        login_payload.len() as i32,
    );
    let mut login_msg = login_frame.encode();
    login_msg.extend_from_slice(&login_payload);
    hex_dump(&login_msg, "  full frame");

    stream.write_all(&login_msg).unwrap();
    println!("Sent LOGIN");

    // Read login response
    let mut header = vec![];
    read_exact(&mut stream, &mut header, 64).unwrap();
    let msg_type = header[4];
    let body_len = i32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
    let resp_code = i32::from_le_bytes([header[10], header[11], header[12], header[13]]);
    let mut body = vec![];
    if body_len > 0 { read_exact(&mut stream, &mut body, body_len).unwrap(); }
    println!("Response: msg_type={} body_len={} resp_code={}", msg_type, body_len, resp_code);
    hex_dump(&body, "  response body");

    if msg_type == dameng_protocol::message::LOGIN_RESPONSE {
        let login_resp = dameng_protocol::message::LoginResponse::from_bytes(&body).unwrap();
        println!("Server name: {}", login_resp.server_name);
        println!("Username: {}", login_resp.username);
        println!("DB name: {}", login_resp.db_name);
    } else {
        println!("ERROR: Expected LOGIN_RESPONSE (163) but got {}", msg_type);
        // Try to decode error message
        if body.len() > 12 {
            let err_len = u32::from_le_bytes([body[8], body.get(9).copied().unwrap_or(0), body.get(10).copied().unwrap_or(0), body.get(11).copied().unwrap_or(0)]) as usize;
            if err_len > 0 && body.len() > 12 + err_len {
                println!("Error message: {}", String::from_utf8_lossy(&body[12..12 + err_len]));
            }
        }
    }
}

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
