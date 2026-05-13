use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn read_n(s: &mut TcpStream, n: usize, timeout_ms: u64) -> Vec<u8> {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    let mut buf = Vec::with_capacity(n);
    let mut tmp = [0u8; 4096];
    while buf.len() < n {
        if std::time::Instant::now() > deadline { break; }
        match s.read(&mut tmp) {
            Ok(0) => break,
            Ok(sz) => buf.extend_from_slice(&tmp[..sz]),
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    buf
}

fn read_timeout(s: &mut TcpStream, timeout_ms: u64) -> Vec<u8> {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if std::time::Instant::now() > deadline { break; }
        match s.read(&mut tmp) {
            Ok(0) => break,
            Ok(sz) => buf.extend_from_slice(&tmp[..sz]),
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    buf
}

fn send_raw(s: &mut TcpStream, data: &[u8], label: &str) {
    println!("--- {} ---", label);
    s.write_all(data).ok();
}

fn hex(label: &str, data: &[u8]) {
    print!("{} ({}b): ", label, data.len());
    for b in data.iter().take(64) { print!("{:02x} ", b); }
    println!();
    if data.len() > 0 { print!("  ascii: "); for &b in data.iter().take(40) { if b >= 32 && b < 127 { print!("{}", b as char) } else { print!(".") } } println!(); }
}

// Frame header: handle(4) + type(1) + rsv(1) + body_len(4) + resp_code(4) + affected(4) + cmp(1) + chk(1) + rsv44 = 64
// Go frame: 20 bytes only
fn make_frame(msg_type: u8, handle: u32, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(20 + payload.len());
    buf.extend_from_slice(&handle.to_le_bytes()); // 0-3
    buf.push(msg_type); // 4
    buf.push(0); // 5: reserved
    buf.extend_from_slice(&(payload.len() as i32).to_le_bytes()); // 6-9
    buf.extend_from_slice(&0i32.to_le_bytes()); // 10-13: resp_code
    buf.extend_from_slice(&0i32.to_le_bytes()); // 14-17: affected
    buf.push(0); // 18: compress
    // compute XOR checksum 0..18
    let xor: u8 = buf[..19].iter().fold(0, |a, b| a ^ b);
    buf.push(xor); // 19: checksum
    buf.extend_from_slice(payload);
    buf
}

fn main() -> std::io::Result<()> {
    let mut s = TcpStream::connect("127.0.0.1:5236")?;
    // Don't set read timeout - we control reads manually
    println!("Connected!");

    // Read startup header
    let hdr = read_n(&mut s, 20, 2000);
    hex("startup hdr", &hdr.max(20));
    let msg_type = hdr[4];
    println!("  msg_type={}", msg_type2name(msg_type));
    let body_len = i32::from_le_bytes([hdr[6], hdr[7], hdr[8], hdr[9]]) as usize;
    println!("  body_len={}", body_lenhed);
    let resp_code = i32::from_le_bytes([hdr[10], hdr[11], hdr[12], hdr[13]]);
    println!("  resp_code={}", resp_code);
    let body = read_n(&mut s, body_len, 2000);
    hex("startup body", &body);
    let rest = read_timeout(&mut s, 200);
    hex("extra after startup", &rest);

    Ok(())
}

fn msg_type2name(t: u8) -> &'static str {
    match t {
        0 => "EXEC_RESp", 5 => "EXEC", 7 => "FETCH", 9 => "ROLLBACK",
        12 => "COMMIT", 52 => "SET_ISOLATION", 91 => "OPE", 
        187 => "ACK", _ => "?",
    }
}
