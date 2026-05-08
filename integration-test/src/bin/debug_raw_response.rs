use std::io::{Read, Write};
use std::net::TcpStream;

fn hex_dump(data: &[u8], label: &str) {
    println!("=== {} ({} bytes) ===", label, data.len());
    for i in (0..data.len()).step_by(16) {
        let chunk = &data[i..(i + 16).min(data.len())];
        let addr = format!("{:04x}", i);
        let hex: String = chunk.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
        let ascii: String = chunk.iter().map(|b| if *b >= 0x20 && *b < 0x7f { *b as char } else { '.' }).collect();
        println!("  {}  {}  {}", addr, hex, ascii);
    }
    println!();
}

fn build_msg(msg_type: u8, handle: i32, payload: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; 64];
    buf[0..4].copy_from_slice(&handle.to_le_bytes());
    buf[4] = msg_type;
    buf[6..10].copy_from_slice(&(payload.len() as i32).to_le_bytes());
    let mut cs: u8 = 0;
    for i in 0..19 { cs ^= buf[i]; }
    buf[19] = cs;
    let mut result = buf;
    result.extend_from_slice(payload);
    result
}

fn read_resp(stream: &mut TcpStream) -> (u8, i32, Vec<u8>) {
    let mut hdr = [0u8; 64];
    stream.read_exact(&mut hdr).unwrap();
    let msg_type = hdr[4];
    let body_len = i32::from_le_bytes([hdr[6], hdr[7], hdr[8], hdr[9]]) as usize;
    let resp_code = i32::from_le_bytes([hdr[10], hdr[11], hdr[12], hdr[13]]);
    let mut body = vec![0u8; body_len];
    stream.read_exact(&mut body).unwrap();
    (msg_type, resp_code, body)
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").unwrap();
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();

    // STARTUP
    let mut startup = vec![0u8; 80];
    startup[0..4].copy_from_slice(&7i32.to_le_bytes());
    startup[4..11].copy_from_slice(b"7.6.0.0");
    startup[11] = 0;
    startup[12..16].copy_from_slice(&64i32.to_le_bytes());
    for i in 0..64 { startup[16 + i] = ((i * 7 + 13) & 0xFF) as u8; }
    stream.write_all(&build_msg(200, 0, &startup)).unwrap();

    let (_st, _sc, startup_resp) = read_resp(&mut stream);
    hex_dump(&startup_resp, "STARTUP RESPONSE");

    let ver_len = u32::from_le_bytes([startup_resp[16], startup_resp[17], startup_resp[18], startup_resp[19]]) as usize;
    let ver_end = 20 + ver_len;
    println!("ver_len={}, ver_end={}, total={}", ver_len, ver_end, startup_resp.len());
    // After version: sentinel(-1), then key_len(4), then key_bytes
    let mut challenge = Vec::new();
    if ver_end + 8 <= startup_resp.len() {
        let key_len = u32::from_le_bytes([
            startup_resp[ver_end+4], startup_resp[ver_end+5],
            startup_resp[ver_end+6], startup_resp[ver_end+7],
        ]) as usize;
        let key_start = ver_end + 8;
        if key_len > 0 && key_start + key_len <= startup_resp.len() {
            challenge = startup_resp[key_start..key_start+key_len].to_vec();
        }
    }
    if challenge.is_empty() {
        println!("No challenge found in startup response (only {} bytes)", startup_resp.len());
    }
    hex_dump(&challenge, "CHALLENGE");

    // LOGIN
    let un = b"SYSDBA"; let pw = b"SYSDBA"; let os = b"Mac OS X"; let host = b"localhost";
    let mut login = Vec::new();
    login.extend_from_slice(&(un.len() as i32).to_le_bytes());
    for i in 0..un.len() {
        login.push(if challenge.is_empty() { un[i] } else { un[i] ^ challenge[i % challenge.len()] });
    }
    login.extend_from_slice(&(pw.len() as i32).to_le_bytes());
    for i in 0..pw.len() {
        login.push(if challenge.is_empty() { pw[i] } else { pw[i] ^ challenge[i % challenge.len()] });
    }
    login.extend_from_slice(&[0, 0, 0, 0]);
    login.extend_from_slice(&(os.len() as i32).to_le_bytes());
    login.extend_from_slice(os);
    login.extend_from_slice(&(host.len() as i32).to_le_bytes());
    login.extend_from_slice(host);
    login.push(0);
    stream.write_all(&build_msg(1, 0, &login)).unwrap();

    let (_st, _sc, login_resp) = read_resp(&mut stream);
    hex_dump(&login_resp, "LOGIN RESPONSE");

    // READY
    stream.write_all(&build_msg(3, 0, &[])).unwrap();
    let (rt, rc, _) = read_resp(&mut stream);
    println!("READY ack: type={} resp={}\n", rt, rc);

    // SELECT 1 FROM DUAL
    let mut ep = b"SELECT 1 FROM DUAL".to_vec();
    ep.push(0);
    stream.write_all(&build_msg(5, 0, &ep)).unwrap();
    let (mt, rc, resp) = read_resp(&mut stream);
    println!("EXEC response: type={} resp={}\n", mt, rc);
    hex_dump(&resp, "EXEC RESPONSE (SELECT 1)");

    // SELECT ID, NAME FROM SAMPLE
    stream.write_all(&build_msg(3, 0, &[])).unwrap();
    let (_, _, _) = read_resp(&mut stream);

    let mut ep2 = b"SELECT ID, NAME FROM SAMPLE".to_vec();
    ep2.push(0);
    stream.write_all(&build_msg(5, 0, &ep2)).unwrap();
    let (mt, rc, resp2) = read_resp(&mut stream);
    println!("EXEC2 response: type={} resp={}\n", mt, rc);
    hex_dump(&resp2, "EXEC RESPONSE (SAMPLE)");

    println!("Done!");
}
