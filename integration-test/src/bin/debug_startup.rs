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
    // Compute XOR checksum of bytes 0-18
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
    
    // Verify checksum
    let mut cs: u8 = 0;
    for i in 0..19 {
        cs ^= buf[i];
    }
    if cs != buf[19] {
        eprintln!("Checksum mismatch: computed={} got={}", cs, buf[19]);
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
    while total < len {
        let mut tmp = vec![0u8; 4096.min(len - total)];
        match stream.read(&mut tmp) {
            Ok(0) => return Err("Connection closed".to_string()),
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                total += n;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == Some(35) => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(e) => return Err(format!("Read error: {}", e)),
        }
    }
    Ok(())
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").expect("Failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    
    // Build startup payload matching the Python driver capture exactly
    let ver = b"7.6.0.0";
    let mut key = [0u8; 64];
    for i in 0..64 {
        key[i] = ((i * 7 + 13) & 0xFF) as u8;
    }
    
    let mut payload = Vec::new();
    payload.extend_from_slice(&(ver.len() as i32).to_le_bytes()); // i32 LE version len
    payload.extend_from_slice(ver); // version string
    payload.push(0); // null terminator
    payload.extend_from_slice(&64i32.to_le_bytes()); // i32 LE key len
    payload.extend_from_slice(&key); // 64 bytes key
    
    println!("=== Building STARTUP message ===");
    hex_dump(&payload, "Startup payload");
    
    let frame = make_frame(200, 0, &payload);
    hex_dump(&frame, "Complete frame");
    
    println!("\n=== Sending STARTUP ===");
    stream.write_all(&frame).expect("Failed to send startup");
    
    // Read frame header
    println!("\n=== Reading response header ===");
    let mut header = vec![];
    read_exact(&mut stream, &mut header, FRAME_HEADER_SIZE).expect("Failed to read header");
    hex_dump(&header, "Response header");
    
    let (msg_type, handle, body_len, resp_code) = parse_frame(&header).expect("Failed to parse frame");
    println!("msg_type={}, handle={}, body_len={}, response_code={}", msg_type, handle, body_len, resp_code);
    
    // Read body
    let body_len = body_len as usize;
    if body_len > 0 {
        println!("\n=== Reading response body ===");
        let mut body = vec![];
        read_exact(&mut stream, &mut body, body_len).expect("Failed to read body");
        hex_dump(&body, "Response body");
        
        // Parse server version
        if body.len() >= 20 {
            let ver_len = u32::from_le_bytes([
                body[16],
                body.get(17).copied().unwrap_or(0),
                body.get(18).copied().unwrap_or(0),
                body.get(19).copied().unwrap_or(0),
            ]) as usize;
            println!("Server version len: {}", ver_len);
            if ver_len > 0 && body.len() > 20 + ver_len {
                println!("Server version: {}", String::from_utf8_lossy(&body[20..20+ver_len]));
            }
        }
    }
    
    println!("\nDone!");
}
