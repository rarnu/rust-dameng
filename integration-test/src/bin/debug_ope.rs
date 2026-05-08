use std::io::{Read, Write};
use std::net::TcpStream;

fn hex_dump(data: &[u8], label: &str) {
    println!("=== {} ({} bytes) ===", label, data.len());
    for i in (0..data.len()).step_by(16) {
        let chunk = &data[i..(i + 16).min(data.len())];
        let addr = format!("{:04x}", i);
        let hex: String = chunk
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ");
        let ascii: String = chunk
            .iter()
            .map(|b| {
                if *b >= 0x20 && *b < 0x7f {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect();
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

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236")
        .expect("connect failed");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();

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
    let (_, _, _) = read_resp(&mut stream);

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
    let (_, _, _) = read_resp(&mut stream);

    // READY
    stream.write_all(&build_msg(3, 0, &[])).unwrap();
    let (_, _, _) = read_resp(&mut stream);

    // === OPE: SELECT 1 FROM DUAL ===
    println!("=== OPE: SELECT 1 FROM DUAL ===");
    let mut sql1 = b"SELECT 1 FROM DUAL".to_vec();
    sql1.push(0);
    stream.write_all(&build_msg(91, 0, &sql1)).unwrap();
    let (mt, rc, resp1) = read_resp(&mut stream);
    println!("OPE SELECT1: type={} resp={}\n", mt, rc);
    hex_dump(&resp1, "OPE SELECT 1 RESPONSE");

    // === OPE: SELECT ID, NAME FROM SAMPLE ===
    println!("=== OPE: SELECT ID, NAME FROM SAMPLE ===");
    let mut sql2 = b"SELECT ID, NAME FROM SAMPLE".to_vec();
    sql2.push(0);
    stream.write_all(&build_msg(91, 0, &sql2)).unwrap();
    let (mt, rc, resp2) = read_resp(&mut stream);
    println!("OPE SAMPLE: type={} resp={}\n", mt, rc);
    hex_dump(&resp2, "OPE SAMPLE RESPONSE");

    // === OPE: INSERT ===
    println!("=== OPE: INSERT ===");
    let mut sql3 = b"INSERT INTO SAMPLE (ID, NAME) VALUES (999, 'TEST')".to_vec();
    sql3.push(0);
    stream.write_all(&build_msg(91, 0, &sql3)).unwrap();
    let (mt, rc, resp3) = read_resp(&mut stream);
    println!("OPE INSERT: type={} resp={}\n", mt, rc);
    hex_dump(&resp3, "OPE INSERT RESPONSE");

    // === OPE: SELECT after INSERT ===
    println!("=== OPE: SELECT after INSERT ===");
    let mut sql4 = b"SELECT ID, NAME FROM SAMPLE".to_vec();
    sql4.push(0);
    stream.write_all(&build_msg(91, 0, &sql4)).unwrap();
    let (mt, rc, resp4) = read_resp(&mut stream);
    println!("OPE SELECT after INSERT: type={} resp={}\n", mt, rc);
    hex_dump(&resp4, "OPE SELECT AFTER INSERT");

    // === OPE: DELETE test row ===
    println!("=== OPE: DELETE test row ===");
    let mut sql5 = b"DELETE FROM SAMPLE WHERE ID = 999".to_vec();
    sql5.push(0);
    stream.write_all(&build_msg(91, 0, &sql5)).unwrap();
    let (mt, rc, resp5) = read_resp(&mut stream);
    println!("OPE DELETE: type={} resp={}\n", mt, rc);
    hex_dump(&resp5, "OPE DELETE RESPONSE");

    println!("Done!");
}
