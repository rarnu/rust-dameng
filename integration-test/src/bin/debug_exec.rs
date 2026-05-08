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
    for i in 0..19 { cs ^= header[i]; }
    header[19] = cs;
    let mut result = header.to_vec();
    result.extend_from_slice(body);
    result
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

fn parse_frame(buf: &[u8]) -> Option<(u8, i32, i32)> {
    if buf.len() < FRAME_HEADER_SIZE { return None; }
    let handle = i32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let msg_type = buf[4];
    let body_len = i32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]);
    let mut cs: u8 = 0;
    for i in 0..19 { cs ^= buf[i]; }
    if cs != buf[19] { return None; }
    Some((msg_type, handle, body_len))
}

fn read_message(stream: &mut TcpStream) -> Result<(u8, Vec<u8>), String> {
    let mut header = vec![];
    read_exact(stream, &mut header, FRAME_HEADER_SIZE)?;
    let (msg_type, _h, body_len) = parse_frame(&header).ok_or("Bad frame")?;
    let body_len = body_len.max(0) as usize;
    let mut body = vec![];
    if body_len > 0 { read_exact(stream, &mut body, body_len)?; }
    println!("  <- msg_type={} body_len={}", msg_type, body_len);
    Ok((msg_type, body))
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
    let frame = make_frame(200, 0, &payload);
    stream.write_all(&frame).unwrap();
    let (_st, body) = read_message(&mut stream).unwrap();
    println!("  server version: 8.1.3.62");

    // Parse challenge
    let mut challenge = vec![0u8; 64];
    if body.len() >= 40 {
        let ver_len = u32::from_le_bytes([body[16], body.get(17).copied().unwrap_or(0), body.get(18).copied().unwrap_or(0), body.get(19).copied().unwrap_or(0)]) as usize;
        let after_ver = 20 + ver_len;
        if after_ver + 8 <= body.len() {
            let key_len = u32::from_le_bytes([body[after_ver+4], body.get(after_ver+5).copied().unwrap_or(0), body.get(after_ver+6).copied().unwrap_or(0), body.get(after_ver+7).copied().unwrap_or(0)]) as usize;
            let key_start = after_ver + 8;
            let copy_len = key_len.min(64).min(body.len() - key_start);
            if copy_len > 0 { challenge[..copy_len].copy_from_slice(&body[key_start..key_start + copy_len]); }
        }
    }

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
    let mut login = Vec::new();
    login.extend_from_slice(&(username.len() as i32).to_le_bytes());
    login.extend_from_slice(&encrypted_un);
    login.extend_from_slice(&(password.len() as i32).to_le_bytes());
    login.extend_from_slice(&encrypted_pw);
    login.extend_from_slice(&[0u8; 4]);
    login.extend_from_slice(&(os_name.len() as i32).to_le_bytes());
    login.extend_from_slice(os_name);
    login.extend_from_slice(&(hostname.len() as i32).to_le_bytes());
    login.extend_from_slice(hostname);
    login.push(0);
    let frame = make_frame(1, 0, &login);
    stream.write_all(&frame).unwrap();
    let (_lt, _lbody) = read_message(&mut stream).unwrap();
    println!("  login OK");

    // === READY ===
    println!("\n=== READY ===");
    // From capture: MSG #5 has payload_len=0, but let's try empty
    let ready_payload: Vec<u8> = vec![];
    let frame = make_frame(3, 0, &ready_payload);
    stream.write_all(&frame).unwrap();
    let (_rt, _rbody) = read_message(&mut stream).unwrap();
    println!("  ready OK");

    // === EXEC: SELECT 1 ===
    println!("\n=== EXEC: SELECT 1 FROM DUAL ===");
    // From capture: MSG #7 has handle=0, msg_type=5, payload_len=34
    // The payload starts with flags then SQL string (null-terminated)
    // Let me try the exact format from the capture
    // @20=0x01 @21=0x01 means bytes at offset 20 are 0x01, 0x01
    // Looking at capture hex:
    // 00 00 00 00 05 00 22 00 ... 00 00 00 27 01 01 00 01 01 00 00 ff ff ff ff ff
    // ff ff 7f 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
    // 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 44 45 4c 45 54 45 20 46 52 4f
    // 4d 20 53 41 4d 50 4c 45 20 57 48 45 52 45 20 49 44 20 3d 20 39 39 38 00
    //
    // The payload format appears to be:
    // Offset 0: 4 bytes zeros
    // Offset 4: 4 bytes zeros (maybe some flags)
    // ...
    // Actually looking at payload offset (after 64-byte header):
    // payload_len=34 for "DELETE FROM SAMPLE WHERE ID = 998"
    // SQL is "DELETE FROM SAMPLE WHERE ID = 998" = 31 chars + null = 32 bytes
    // So 34-32 = 2 bytes of header before SQL? No...
    //
    // Let me look at the raw bytes more carefully:
    // 0000: 00 00 00 00 05 00 22 00 00 00 00 00 00 00 00 00  (frame header)
    // 0010: 00 00 00 27 01 01 00 01 01 00 00 ff ff ff ff ff  (frame header continued)
    // 0020: ff ff 7f 00 00 00 00 00 00 00 00 00 00 00 00 00  (frame header continued)
    // 0030: 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00  (frame header continued)
    // 0040: 44 45 4c 45 54 45 20 46 52 4f 4d 20 53 41 4d 50  "DELETE FROM SAMP"
    // 0050: 4c 45 20 57 48 45 52 45 20 49 44 20 3d 20 39 39  "LE WHERE ID = 99"
    // 0060: 38 00                                             "8."
    //
    // So payload starts at offset 0x40 (64) which is the start of the frame body
    // Wait, body_len=34 (0x22), so payload is exactly 34 bytes:
    // 44 45 4c 45 54 45 20 46 52 4f 4d 20 53 41 4d 50
    // 4c 45 20 57 48 45 52 45 20 49 44 20 3d 20 39 39
    // 38 00
    // That's just the SQL + null terminator! 31 + 1 = 32... but body_len=34.
    //
    // Actually wait - let me recount. The frame header is 64 bytes (0x00-0x3F).
    // Body starts at 0x40. Let me count from 0x40:
    // 0x40: 44 45 4c 45 54 45 20 46 52 4f 4d 20 53 41 4d 50  "DELETE FROM SAMP"
    // 0x50: 4c 45 20 57 48 45 52 45 20 49 44 20 3d 20 39 39  "LE WHERE ID = 99"
    // 0x60: 38 00                                             "8."
    // That's 0x62 - 0x40 = 0x22 = 34 bytes. Yes!
    //
    // So the EXEC payload is: SQL string + null terminator + 2 extra bytes?
    // "DELETE FROM SAMPLE WHERE ID = 998" = 31 chars
    // 31 + 1 (null) = 32. But payload is 34. So 2 extra bytes.
    //
    // Hmm, let me look at MSG #11 (INSERT):
    // payload_len=44 (0x2c)
    // SQL = "INSERT INTO SAMPLE (ID, NAME) VALUES (?, ?)" = 41 chars
    // 41 + 1 (null) + 2 = 44. Same pattern!
    //
    // So EXEC payload = SQL + null + 2 bytes of something?
    // Or maybe SQL is padded to 4-byte boundary?
    // 31 + 1 = 32, padded to 36? No, 34.
    //
    // Actually, looking at MSG #7 more carefully:
    // Frame header at 0x00: handle=0, msg_type=5, body_len=34 (0x22)
    // Body at 0x40: "DELETE FROM SAMPLE WHERE ID = 998\0" + 2 bytes
    //
    // Wait, let me recheck the capture format. The hex dump shows the FULL packet
    // including the 64-byte frame header. The body_len=0x22=34 means the body
    // after the 64-byte header is 34 bytes long.
    //
    // Actually from the capture the EXEC payload_len field says 34 which matches.
    // And the payload IS just the SQL + null + padding.
    // "DELETE FROM SAMPLE WHERE ID = 998" is 31 bytes + 1 null = 32 bytes.
    // 34 - 32 = 2 extra bytes. These must be part of the SQL encoding.
    //
    // Let me look at MSG #15 (SELECT):
    // payload_len=34 (0x22)
    // SQL = "SELECT * FROM SAMPLE WHERE ID = ?" = 32 chars
    // 32 + 1 (null) + 1 = 34. Maybe just null-terminated with some alignment.
    //
    // For now, let me just try sending the SQL + null terminator and see if it works.

    // Try simple format: SQL + null terminator
    let sql = "SELECT 1 FROM DUAL";
    let mut exec_payload: Vec<u8> = Vec::new();
    exec_payload.extend_from_slice(sql.as_bytes());
    exec_payload.push(0); // null terminator
    hex_dump(&exec_payload, "EXEC payload attempt 1");

    let frame = make_frame(5, 0, &exec_payload);
    stream.write_all(&frame).unwrap();
    println!("  -> sent EXEC");
    match read_message(&mut stream) {
        Ok((resp_type, resp_body)) => {
            hex_dump(&resp_body, "EXEC response body");
            // Parse the response
            if resp_type == 0 {
                // EXEC_RESPONSE has columns and data
                // Column metadata: 4 bytes col_name_len + name, 4 bytes type_name_len + name, ...
                println!("  Got EXEC_RESPONSE with {} bytes", resp_body.len());
                // Try to parse column info
                if resp_body.len() > 8 {
                    let col_count = u32::from_le_bytes([resp_body[0], resp_body[1], resp_body[2], resp_body[3]]) as usize;
                    let row_count = u32::from_le_bytes([resp_body[4], resp_body[5], resp_body[6], resp_body[7]]) as usize;
                    println!("  Columns: {}, Rows: {}", col_count, row_count);
                }
            } else {
                println!("  Response type: {}", resp_type);
            }
        }
        Err(e) => {
            println!("  EXEC error: {}", e);
        }
    }

    println!("\n=== Done ===");
}
