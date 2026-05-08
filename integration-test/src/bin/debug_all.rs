use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn make_frame(msg_type: u8, handle: i32, body: &[u8]) -> Vec<u8> {
    let body_len = body.len() as i32;
    let mut header = [0u8; 64];
    header[0..4].copy_from_slice(&handle.to_le_bytes());
    header[4] = msg_type;
    header[6..10].copy_from_slice(&body_len.to_le_bytes());
    let mut cs: u8 = 0;
    for i in 0..19 { cs ^= header[i]; }
    header[19] = cs;
    let mut result = header.to_vec();
    result.extend_from_slice(body);
    result
}

fn read_exact(stream: &mut TcpStream, buf: &mut Vec<u8>, len: usize) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut total = 0;
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
    let ver = b"7.6.0.0";
    let mut key = [0u8; 64];
    for i in 0..64 { key[i] = ((i * 7 + 13) & 0xFF) as u8; }
    let mut payload = Vec::new();
    payload.extend_from_slice(&(ver.len() as i32).to_le_bytes());
    payload.extend_from_slice(ver);
    payload.push(0);
    payload.extend_from_slice(&64i32.to_le_bytes());
    payload.extend_from_slice(&key);
    hex_dump(&payload, "Startup payload");
    let frame = make_frame(200, 0, &payload);
    stream.write_all(&frame).unwrap();
    println!("Sent STARTUP ({} bytes total)", frame.len());

    // Read response
    let mut header = vec![];
    read_exact(&mut stream, &mut header, 64).unwrap();
    let body_len = i32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
    let resp_code = i32::from_le_bytes([header[10], header[11], header[12], header[13]]);
    println!("  -> msg_type={} body_len={} resp_code={}", header[4], body_len, resp_code);
    let mut body = vec![];
    if body_len > 0 { read_exact(&mut stream, &mut body, body_len).unwrap(); }
    hex_dump(&body, "Startup response body");

    // Parse version from response
    let ver_len = if body.len() > 19 {
        u32::from_le_bytes([body[16], body.get(17).copied().unwrap_or(0), body.get(18).copied().unwrap_or(0), body.get(19).copied().unwrap_or(0)]) as usize
    } else { 0 };
    let server_ver = if ver_len > 0 && body.len() > 20 + ver_len {
        String::from_utf8_lossy(&body[20..20 + ver_len]).to_string()
    } else { "unknown".to_string() };
    println!("  Server version: {}", server_ver);

    // Parse challenge from response
    let mut challenge = vec![0u8; 64];
    let after_ver = 20 + ver_len;
    if body.len() > after_ver + 8 {
        let key_len = u32::from_le_bytes([body[after_ver+4], body.get(after_ver+5).copied().unwrap_or(0), body.get(after_ver+6).copied().unwrap_or(0), body.get(after_ver+7).copied().unwrap_or(0)]) as usize;
        let key_start = after_ver + 8;
        let copy_len = key_len.min(64).min(body.len() - key_start);
        if copy_len > 0 { challenge[..copy_len].copy_from_slice(&body[key_start..key_start + copy_len]); }
    }
    println!("  Challenge bytes (first 8): {}", challenge[..8].iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" "));

    // === LOGIN ===
    println!("\n=== LOGIN ===");
    let username = b"SYSDBA";
    let password = b"SYSDBA";
    let mut encrypted_un = Vec::with_capacity(username.len());
    for i in 0..username.len() { encrypted_un.push(username[i] ^ challenge[i % challenge.len()]); }
    let mut encrypted_pw = Vec::with_capacity(password.len());
    for i in 0..password.len() { encrypted_pw.push(password[i] ^ challenge[i % challenge.len()]); }
    let os_name = b"Mac OS X";
    let hostname = b"localhost";

    let mut login_payload = Vec::new();
    login_payload.extend_from_slice(&(username.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(&encrypted_un);
    login_payload.extend_from_slice(&(password.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(&encrypted_pw);
    login_payload.extend_from_slice(&[0u8; 4]);
    login_payload.extend_from_slice(&(os_name.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(os_name);
    login_payload.extend_from_slice(&(hostname.len() as i32).to_le_bytes());
    login_payload.extend_from_slice(hostname);
    login_payload.push(0);

    hex_dump(&login_payload, "Login payload");
    let frame = make_frame(1, 0, &login_payload);
    stream.write_all(&frame).unwrap();
    println!("Sent LOGIN ({} bytes total)", frame.len());

    // Read login response
    let mut header = vec![];
    read_exact(&mut stream, &mut header, 64).unwrap();
    let body_len = i32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
    let resp_code = i32::from_le_bytes([header[10], header[11], header[12], header[13]]);
    println!("  -> msg_type={} body_len={} resp_code={}", header[4], body_len, resp_code);
    let mut body = vec![];
    if body_len > 0 { read_exact(&mut stream, &mut body, body_len).unwrap(); }
    hex_dump(&body, "Login response body");

    // Parse login response
    if body.len() > 0x20 {
        let sn_len = u32::from_le_bytes([body[0x10], body.get(0x11).copied().unwrap_or(0), body.get(0x12).copied().unwrap_or(0), body.get(0x13).copied().unwrap_or(0)]) as usize;
        let server_name = if sn_len > 0 && body.len() > 0x14 + sn_len {
            String::from_utf8_lossy(&body[0x14..0x14 + sn_len]).to_string()
        } else { String::new() };
        println!("  Server name: {}", server_name);
        let un_off = 0x14 + sn_len;
        if body.len() > un_off + 4 {
            let un_len = u32::from_le_bytes([body[un_off], body.get(un_off+1).copied().unwrap_or(0), body.get(un_off+2).copied().unwrap_or(0), body.get(un_off+3).copied().unwrap_or(0)]) as usize;
            if un_len > 0 && body.len() > un_off + 4 + un_len {
                println!("  Username: {}", String::from_utf8_lossy(&body[un_off+4..un_off+4+un_len]));
            }
        }
    }

    // === READY ===
    println!("\n=== READY ===");
    let ready_payload: Vec<u8> = vec![];
    let frame = make_frame(3, 0, &ready_payload);
    stream.write_all(&frame).unwrap();
    println!("Sent READY ({} bytes total)", frame.len());

    let mut header = vec![];
    read_exact(&mut stream, &mut header, 64).unwrap();
    let body_len = i32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
    println!("  -> msg_type={} body_len={}", header[4], body_len);
    let mut body = vec![];
    if body_len > 0 { read_exact(&mut stream, &mut body, body_len).unwrap(); }
    if body.len() > 7 {
        let msg_off = body.len() - 7;
        let msg_len = u32::from_le_bytes([body[msg_off], body.get(msg_off+1).copied().unwrap_or(0), body.get(msg_off+2).copied().unwrap_or(0), body.get(msg_off+3).copied().unwrap_or(0)]) as usize;
        if msg_len > 0 && body.len() >= msg_off + 4 + msg_len {
            println!("  Message: {}", String::from_utf8_lossy(&body[msg_off+4..msg_off+4+msg_len]));
        }
    }

    // === EXEC: SELECT 1 FROM DUAL ===
    println!("\n=== EXEC: SELECT 1 FROM DUAL ===");
    let sql = b"SELECT 1 FROM DUAL";
    let mut exec_payload: Vec<u8> = Vec::new();
    exec_payload.extend_from_slice(sql);
    exec_payload.push(0);
    hex_dump(&exec_payload, "EXEC payload");
    let frame = make_frame(5, 0, &exec_payload);
    stream.write_all(&frame).unwrap();
    println!("Sent EXEC ({} bytes total)", frame.len());

    let mut header = vec![];
    read_exact(&mut stream, &mut header, 64).unwrap();
    let body_len = i32::from_le_bytes([header[6], header[7], header[8], header[9]]) as usize;
    let resp_code = i32::from_le_bytes([header[10], header[11], header[12], header[13]]);
    println!("  -> msg_type={} body_len={} resp_code={}", header[4], body_len, resp_code);
    let mut body = vec![];
    if body_len > 0 { read_exact(&mut stream, &mut body, body_len).unwrap(); }
    hex_dump(&body, "EXEC response body");

    // Parse EXEC response
    if body.len() > 8 {
        // Column metadata starts after header fields
        let col_count = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
        let row_count = u32::from_le_bytes([body[4], body[5], body[6], body[7]]) as usize;
        println!("  Columns: {}, Rows: {}", col_count, row_count);
        // Column 0 metadata at offset 8
        if body.len() > 16 {
            let col_name_len = u32::from_le_bytes([body[8], body[9], body[10], body[11]]) as usize;
            if col_name_len > 0 && body.len() > 12 + col_name_len {
                println!("  Column 0 name: {}", String::from_utf8_lossy(&body[12..12+col_name_len]));
            }
            let type_name_len = u32::from_le_bytes([body[12+col_name_len], body.get(12+col_name_len+1).copied().unwrap_or(0), body.get(12+col_name_len+2).copied().unwrap_or(0), body.get(12+col_name_len+3).copied().unwrap_or(0)]) as usize;
            if type_name_len > 0 {
                let type_start = 12 + col_name_len + 4;
                if body.len() > type_start + type_name_len {
                    println!("  Column 0 type: {}", String::from_utf8_lossy(&body[type_start..type_start+type_name_len]));
                }
            }
        }
    }

    println!("\n=== All tests passed! ===");
}
