// Trace parser logic against the raw V$VERSION OPE response bytes
use std::io::{Read, Write};
use std::net::TcpStream;

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
    let mut stream = TcpStream::connect("127.0.0.1:5236").expect("connect failed");
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

    // LOGIN
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

    // OPE: SELECT * FROM V$VERSION
    println!("=== OPE: SELECT * FROM V$VERSION ===");
    let mut sql = b"SELECT * FROM V$VERSION".to_vec();
    sql.push(0);
    stream.write_all(&build_msg(91, 0, &sql)).unwrap();
    let (msg_type, resp_code, body) = read_resp(&mut stream);
    println!(
        "OPE response: type={} resp={} body_len={}\n",
        msg_type,
        resp_code,
        body.len()
    );

    let data = &body;

    // === Parse header ===
    let row_count = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    println!("Header: row_count={}", row_count);

    // === Parse first column header ===
    let first_col_type = i32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let first_nullable = u16::from_le_bytes([data[20], data[21]]);
    let _display = u16::from_le_bytes([data[22], data[23]]);
    let col_count = u16::from_le_bytes([data[24], data[25]]);
    let type_name_len = u16::from_le_bytes([data[26], data[27]]) as usize;
    let table_name_len = u16::from_le_bytes([data[28], data[29]]) as usize;
    let schema_name_len = u16::from_le_bytes([data[30], data[31]]) as usize;
    println!(
        "Col1: type={} nullable={} col_count={} type_name={} table={} schema={}",
        first_col_type,
        first_nullable,
        col_count,
        type_name_len,
        table_name_len,
        schema_name_len
    );

    // === Column strings at offset 32 ===
    let offset = 32usize;
    let known_str_len = type_name_len + table_name_len + schema_name_len;
    println!(
        "known_str_len = {} (type={} + table={} + schema={})",
        known_str_len, type_name_len, table_name_len, schema_name_len
    );

    // Print raw string area byte by byte
    println!("\n--- Raw string area (offset {} to end) ---", offset);
    for i in 0..(data.len().saturating_sub(offset)) {
        let b = data[offset + i];
        if b == 0 {
            print!("[00]");
        } else if b >= 0x20 && b < 0x7f {
            print!("  {}", b as char);
        } else {
            print!("{:02x}", b);
        }
        if (i + 1) % 16 == 0 {
            println!(" <- offset {}", offset + i);
        }
    }
    println!("\n");

    // Find first null
    let first_null = data[offset..].iter().position(|&b| b == 0).unwrap_or(0);
    println!(
        "First null at relative offset {} (absolute {})",
        first_null,
        offset + first_null
    );

    // Try reading col_name by assuming null-terminated strings
    let col_name = String::from_utf8_lossy(&data[offset..offset + first_null]);
    println!(
        "Col1 name (until first null): '{}' ({})",
        col_name, first_null
    );

    // After first null: read type_name, table_name, schema_name
    let mut cur = offset + first_null;
    // Skip the null itself
    cur += 1;
    println!("\nAfter first null, cursor at {}", cur);
    println!(
        "Bytes at cursor: {:02x?}",
        &data[cur..cur + 32.min(data.len() - cur)]
    );

    // Read type_name (known len)
    let tn = String::from_utf8_lossy(&data[cur..cur + type_name_len.min(data.len() - cur)]);
    println!("type_name at {}: '{}' ({})", cur, tn, type_name_len);
    cur += type_name_len;

    // Read table_name
    let tb = String::from_utf8_lossy(&data[cur..cur + table_name_len.min(data.len() - cur)]);
    println!("table_name at {}: '{}' ({})", cur, tb, table_name_len);
    cur += table_name_len;

    // Read schema_name
    let sc = String::from_utf8_lossy(&data[cur..cur + schema_name_len.min(data.len() - cur)]);
    println!("schema_name at {}: '{}' ({})", cur, sc, schema_name_len);
    cur += schema_name_len;

    println!("\nAfter first col strings, cursor at {}", cur);
    println!(
        "Remaining bytes: {} ({:?})",
        data.len() - cur,
        &data[cur..cur + 48.min(data.len() - cur)]
    );

    // Scan for between-cols metadata or row data
    println!("\n--- Scanning for between-cols / row data ---");
    let mut scan = cur;
    while scan < data.len() {
        let b = data[scan];
        let marker = if b >= 12 && b <= 20 {
            "<row_marker>"
        } else {
            ""
        };
        if !marker.is_empty() {
            println!(
                "  offset {}: byte=0x{:02x} {}",
                scan, b, marker
            );
            // Try to parse as row
            if scan + 12 <= data.len() {
                let rec_id = u32::from_le_bytes([
                    data[scan + 2],
                    data[scan + 3],
                    data[scan + 4],
                    data[scan + 5],
                ]);
                println!(
                    "    potential row: header_size=0x{:02x} flags=0x{:02x} rec_id={}",
                    b,
                    data[scan + 1],
                    rec_id
                );
                // Show column offsets
                for c in 0..col_count as usize {
                    let off_pos = scan + 10 + c * 2;
                    if off_pos + 2 <= data.len() {
                        let off =
                            u16::from_le_bytes([data[off_pos], data[off_pos + 1]]) as usize;
                        let val_abs = scan + off;
                        let val_end = if val_abs + 2 <= data.len() {
                            let vlen =
                                u16::from_le_bytes([data[val_abs], data[val_abs + 1]]) as usize;
                            val_abs + 2 + vlen
                        } else {
                            0
                        };
                        let val_str = if val_abs + 2 <= data.len() {
                            let vlen =
                                u16::from_le_bytes([data[val_abs], data[val_abs + 1]]) as usize;
                            if val_abs + 2 + vlen <= data.len() {
                                String::from_utf8_lossy(&data[val_abs + 2..val_abs + 2 + vlen])
                                    .to_string()
                            } else {
                                "[truncated]".to_string()
                            }
                        } else {
                            "[oob]".to_string()
                        };
                        println!(
                            "    col{}: offset={} val_end={} value='{}'",
                            c, off, val_end, val_str
                        );
                    }
                }
                // Calculate actual row end
                let mut max_val_end = scan;
                for c in 0..col_count as usize {
                    let off_pos = scan + 10 + c * 2;
                    if off_pos + 2 <= data.len() {
                        let off =
                            u16::from_le_bytes([data[off_pos], data[off_pos + 1]]) as usize;
                        let val_abs = scan + off;
                        if val_abs + 2 <= data.len() {
                            let vlen =
                                u16::from_le_bytes([data[val_abs], data[val_abs + 1]]) as usize;
                            let end = val_abs + 2 + vlen;
                            if end > max_val_end {
                                max_val_end = end;
                            }
                        }
                    }
                }
                println!(
                    "    actual_row_size: {} (from {} to {})",
                    max_val_end - scan,
                    scan,
                    max_val_end
                );
                scan = max_val_end;
                continue;
            }
        }
        scan += 1;
    }
}
