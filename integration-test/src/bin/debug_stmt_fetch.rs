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
    for i in 0..19 {
        cs ^= buf[i];
    }
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

fn connect_and_login(stream: &mut TcpStream) {
    // STARTUP
    let mut startup = vec![0u8; 80];
    startup[0..4].copy_from_slice(&7i32.to_le_bytes());
    startup[4..11].copy_from_slice(b"7.6.0.0");
    startup[11] = 0;
    startup[12..16].copy_from_slice(&64i32.to_le_bytes());
    for i in 0..64 {
        startup[16 + i] = ((i * 7 + 13) & 0xFF) as u8;
    }
    stream.write_all(&build_msg(200, 0, &startup)).unwrap();
    let (_, _, _) = read_resp(stream);

    // LOGIN (plaintext)
    let un = b"SYSDBA";
    let pw = b"SYSDBA";
    let os = b"Mac OS X";
    let host = b"localhost";
    let mut login = Vec::new();
    login.extend_from_slice(&(un.len() as i32).to_le_bytes());
    login.extend_from_slice(un);
    login.extend_from_slice(&(pw.len() as i32).to_le_bytes());
    login.extend_from_slice(pw);
    login.extend_from_slice(&[0, 0, 0, 0]);
    login.extend_from_slice(&(os.len() as i32).to_le_bytes());
    login.extend_from_slice(os);
    login.extend_from_slice(&(host.len() as i32).to_le_bytes());
    login.extend_from_slice(host);
    login.push(0);
    stream.write_all(&build_msg(1, 0, &login)).unwrap();
    let (_, _, _) = read_resp(stream);

    // READY
    stream.write_all(&build_msg(3, 0, &[])).unwrap();
    let (_, _, _) = read_resp(stream);
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").unwrap();
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();

    connect_and_login(&mut stream);

    // === Method A: OPTIMIZED_PREPARE_EXEC (type 91) ===
    println!("=== Method A: OPTIMIZED_PREPARE_EXEC (type 91) ===");
    
    // Try sending SQL directly as payload
    let sql = b"SELECT 1 FROM DUAL";
    let mut payload = sql.to_vec();
    payload.push(0);
    stream.write_all(&build_msg(91, 0, &payload)).unwrap();
    let (mt, rc, resp) = read_resp(&mut stream);
    println!("OPE response: type={} resp={}", mt, rc);
    hex_dump(&resp, "OPE RESPONSE");

    // === Method B: STATEMENT_PREPARE with SQL in payload ===
    println!("\n=== Method B: STATEMENT_PREPARE with SQL ===");
    
    connect_and_login(&mut stream);
    
    // STATEMENT_PREPARE with SQL + null terminator
    let mut sp_payload = sql.to_vec();
    sp_payload.push(0);
    stream.write_all(&build_msg(3, 0, &sp_payload)).unwrap();
    let (mt, rc, sp_resp) = read_resp(&mut stream);
    println!("PREPARE response: type={} resp={}", mt, rc);
    hex_dump(&sp_resp, "STATEMENT_PREPARE RESPONSE");

    // Check if handle is in response
    if sp_resp.len() >= 4 {
        // Try different offsets for statement handle
        for off in [0, 4, 8, 12, 16, 20] {
            if off + 4 <= sp_resp.len() {
                let val = u32::from_le_bytes([
                    sp_resp[off], sp_resp[off+1], sp_resp[off+2], sp_resp[off+3],
                ]);
                println!("  Potential handle at offset {}: {} (0x{:08x})", off, val, val);
            }
        }
    }

    // === Method C: EXEC with handle=0, then FETCH with handle=0 ===
    println!("\n=== Method C: EXEC+FETCH with handle=0 ===");
    
    connect_and_login(&mut stream);
    
    stream.write_all(&build_msg(3, 0, &[])).unwrap();
    let (_, _, _) = read_resp(&mut stream);

    let mut ep = sql.to_vec();
    ep.push(0);
    stream.write_all(&build_msg(5, 0, &ep)).unwrap();
    let (mt, rc, exec_resp) = read_resp(&mut stream);
    println!("EXEC response: type={} resp={}", mt, rc);
    hex_dump(&exec_resp, "EXEC RESPONSE");

    // Try FETCH with different handle values
    for handle in [0, 1, 2, 3] {
        stream.write_all(&build_msg(3, 0, &[])).unwrap();
        let (_, _, _) = read_resp(&mut stream);

        let fetch_payload = [1u8, 0, 0, 0];
        stream.write_all(&build_msg(7, handle, &fetch_payload)).unwrap();
        let (ft, fc, fetch_resp) = read_resp(&mut stream);
        println!("FETCH(handle={}): type={} resp={}", handle, ft, fc);
        if fc >= 0 {
            hex_dump(&fetch_resp, &format!("FETCH RESPONSE handle={}", handle));
            break;
        }
        if fetch_resp.len() < 16 {
            hex_dump(&fetch_resp, &format!("FETCH ERR handle={}", handle));
        }
    }

    // === Method D: FETCH_RESULT_SET (type 44) ===
    println!("\n=== Method D: FETCH_RESULT_SET (type 44) ===");
    
    connect_and_login(&mut stream);
    
    stream.write_all(&build_msg(3, 0, &[])).unwrap();
    let (_, _, _) = read_resp(&mut stream);

    let mut ep = sql.to_vec();
    ep.push(0);
    stream.write_all(&build_msg(5, 0, &ep)).unwrap();
    let (mt, rc, exec_resp) = read_resp(&mut stream);
    println!("EXEC response: type={} resp={}", mt, rc);

    // Try FETCH_RESULT_SET (type 44)
    let frs_payload = [0u8; 4];
    stream.write_all(&build_msg(44, 0, &frs_payload)).unwrap();
    let (ft, fc, frs_resp) = read_resp(&mut stream);
    println!("FETCH_RESULT_SET: type={} resp={}", ft, fc);
    hex_dump(&frs_resp, "FETCH_RESULT_SET RESPONSE");

    println!("\nDone!");
}
