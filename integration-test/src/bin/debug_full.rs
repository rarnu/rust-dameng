use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const FRAME_HEADER_SIZE: usize = 64;

fn make_frame(msg_type: u8, handle: i32, body: &[u8]) -> Vec<u8> {
    let body_len = body.len() as i32;
    let mut header = [0u8; FRAME_HEADER_SIZE];
    header[0..4].copy_from_slice(&handle.to_le_bytes());
    header[4] = msg_type;
    header[6..10].copy_from_slice(&body_len.to_le_bytes());
    let mut cs: u8 = 0;
    for i in 0..19 {
        cs ^= header[i];
    }
    header[19] = cs;
    let mut result = header.to_vec();
    result.extend_from_slice(body);
    result
}

fn parse_frame(buf: &[u8]) -> Option<(u8, i32, i32, i32)> {
    if buf.len() < FRAME_HEADER_SIZE {
        return None;
    }
    let handle = i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let msg_type = buf[4];
    let body_len = i32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]);
    let response_code = i32::from_le_bytes([buf[10], buf[11], buf[12], buf[13]]);
    let mut cs: u8 = 0;
    for i in 0..19 {
        cs ^= buf[i];
    }
    if cs != buf[19] {
        eprintln!("Checksum mismatch");
        return None;
    }
    Some((msg_type, handle, body_len, response_code))
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

fn read_exact(stream: &mut TcpStream, buf: &mut Vec<u8>, len: usize) -> Result<(), String> {
    let mut total = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while total < len {
        let mut tmp = vec![0u8; 4096.min(len - total)];
        match stream.read(&mut tmp) {
            Ok(0) => return Err("Connection closed".to_string()),
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                total += n;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == Some(35) => {
                if std::time::Instant::now() > deadline {
                    return Err("Timeout".to_string());
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => return Err(format!("Read error: {}", e)),
        }
    }
    Ok(())
}

fn read_message(stream: &mut TcpStream) -> Result<(u8, i32, i32, Vec<u8>), String> {
    let mut header = vec![];
    read_exact(stream, &mut header, FRAME_HEADER_SIZE)?;
    let (msg_type, handle, body_len, resp_code) = parse_frame(&header).ok_or("Bad frame")?;
    println!("  -> msg_type={} handle={} body_len={} resp_code={}", msg_type, handle, body_len, resp_code);
    let body_len = body_len.max(0) as usize;
    let mut body = vec![];
    if body_len > 0 {
        read_exact(stream, &mut body, body_len)?;
    }
    Ok((msg_type, handle, resp_code, body))
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").expect("Failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    // === STARTUP ===
    println!("\n=== Sending STARTUP ===");
    let ver = b"7.6.0.0";
    let mut key = [0u8; 64];
    for i in 0..64 {
        key[i] = ((i * 7 + 13) & 0xFF) as u8;
    }
    let mut payload = Vec::new();
    payload.extend_from_slice(&(ver.len() as i32).to_le_bytes());
    payload.extend_from_slice(ver);
    payload.push(0);
    payload.extend_from_slice(&64i32.to_le_bytes());
    payload.extend_from_slice(&key);
    hex_dump(&payload, "Startup payload");
    let frame = make_frame(200, 0, &payload);
    stream.write_all(&frame).expect("Failed to send startup");

    // Read startup response
    println!("\n=== Reading startup response ===");
    let (st, _h, resp_code, body) = read_message(&mut stream).expect("Failed to read startup response");
    hex_dump(&body, "Startup response body");
    println!("Startup response: msg_type={} resp_code={}", st, resp_code);

    // Parse server version and challenge from response
    let mut challenge = vec![0u8; 64];
    if body.len() >= 40 {
        let ver_len = u32::from_le_bytes([
            body[16], body.get(17).copied().unwrap_or(0),
            body.get(18).copied().unwrap_or(0), body.get(19).copied().unwrap_or(0),
        ]) as usize;
        let after_ver = 20 + ver_len;
        if after_ver + 8 <= body.len() {
            let key_len = u32::from_le_bytes([
                body[after_ver + 4], body.get(after_ver + 5).copied().unwrap_or(0),
                body.get(after_ver + 6).copied().unwrap_or(0), body.get(after_ver + 7).copied().unwrap_or(0),
            ]) as usize;
            let key_start = after_ver + 8;
            let copy_len = key_len.min(64).min(body.len() - key_start);
            if copy_len > 0 {
                challenge[..copy_len].copy_from_slice(&body[key_start..key_start + copy_len]);
            }
        }
    }
    println!("Extracted challenge (first 16 bytes): {}", challenge[..16].iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));

    // === LOGIN ===
    println!("\n=== Building LOGIN ===");
    let username = b"SYSDBA";
    let password = b"SYSDBA";
    let os_name = b"Mac OS X";
    let hostname = b"localhost";

    // XOR encrypt with challenge
    let mut encrypted_username = Vec::with_capacity(username.len());
    for i in 0..username.len() {
        encrypted_username.push(username[i] ^ challenge[i % challenge.len()]);
    }
    let mut encrypted_password = Vec::with_capacity(password.len());
    for i in 0..password.len() {
        encrypted_password.push(password[i] ^ challenge[i % challenge.len()]);
    }

    let mut login_payload = Vec::new();
    // Username
    login_payload.extend_from_slice(&(username.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(&encrypted_username);
    // Password
    login_payload.extend_from_slice(&(password.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(&encrypted_password);
    // Separator (4 bytes of zeros - from capture)
    login_payload.extend_from_slice(&[0u8; 4]);
    // OS name
    login_payload.extend_from_slice(&(os_name.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(os_name);
    // Hostname + null
    login_payload.extend_from_slice(&(hostname.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(hostname);
    login_payload.push(0);

    hex_dump(&login_payload, "Login payload");
    let login_frame = make_frame(1, 0, &login_payload);
    println!("\n=== Sending LOGIN ===");
    stream.write_all(&login_frame).expect("Failed to send login");

    // Read login response
    println!("\n=== Reading login response ===");
    match read_message(&mut stream) {
        Ok((lt, _h, resp_code, body)) => {
            hex_dump(&body, "Login response body");
            println!("Login response: msg_type={} resp_code={}", lt, resp_code);
            // Parse server name
            if body.len() > 0x48 {
                let sn_len = u32::from_le_bytes([
                    body[0x44], body.get(0x45).copied().unwrap_or(0),
                    body.get(0x46).copied().unwrap_or(0), body.get(0x47).copied().unwrap_or(0),
                ]) as usize;
                if sn_len > 0 && body.len() > 0x48 + sn_len {
                    println!("Server name: {}", String::from_utf8_lossy(&body[0x48..0x48 + sn_len]));
                    // Username
                    let un_off = 0x48 + sn_len;
                    if body.len() > un_off + 4 {
                        let un_len = u32::from_le_bytes([
                            body[un_off], body.get(un_off + 1).copied().unwrap_or(0),
                            body.get(un_off + 2).copied().unwrap_or(0), body.get(un_off + 3).copied().unwrap_or(0),
                        ]) as usize;
                        if un_len > 0 && body.len() > un_off + 4 + un_len {
                            println!("Username: {}", String::from_utf8_lossy(&body[un_off + 4..un_off + 4 + un_len]));
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("Login response error: {}", e);
        }
    }

    // === READY ===
    println!("\n=== Sending READY ===");
    let ready_payload: Vec<u8> = vec![];
    let ready_frame = make_frame(3, 0, &ready_payload);
    stream.write_all(&ready_frame).expect("Failed to send ready");
    match read_message(&mut stream) {
        Ok((rt, _h, resp_code, body)) => {
            println!("READY response: msg_type={} resp_code={}", rt, resp_code);
            if body.len() > 7 {
                let msg_len = u32::from_le_bytes([
                    body[body.len() - 7], body.get(body.len() - 6).copied().unwrap_or(0),
                    body.get(body.len() - 5).copied().unwrap_or(0), body.get(body.len() - 4).copied().unwrap_or(0),
                ]) as usize;
                if msg_len > 0 && body.len() >= body.len() - 7 + msg_len {
                    println!("Message: {}", String::from_utf8_lossy(&body[body.len() - 7..]));
                }
            }
        }
        Err(e) => {
            println!("READY response error: {}", e);
        }
    }

    println!("\n=== Done ===");
}
