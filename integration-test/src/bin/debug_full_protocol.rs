use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use bytes::{BufMut, BytesMut};
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use dameng_protocol::message::*;

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

fn hex_dump(data: &[u8], prefix: &str) {
    println!("{} ({} bytes):", prefix, data.len());
    for i in (0..data.len()).step_by(16) {
        let chunk = &data[i..(i + 16).min(data.len())];
        let hex = chunk.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(" ");
        let ascii = chunk.iter().map(|b| if *b >= 32 && *b < 127 { *b as char } else { '.' }).collect::<String>();
        println!("  {:04x}: {:<48}  {}", i, hex, ascii);
    }
}

fn read_message(stream: &mut TcpStream) -> Result<(Frame, Vec<u8>), String> {
    let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + 4096);
    loop {
        if buf.len() >= FRAME_HEADER_SIZE { break; }
        let mut tmp = vec![0u8; 1024];
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {}", e))?;
        if n == 0 { return Err("Connection closed".to_string()); }
        buf.extend_from_slice(&tmp[..n]);
    }
    let frame = Frame::parse(&mut buf).map_err(|e| format!("parse frame: {:?}", e))?;
    let body_len = frame.body_len.max(0) as usize;
    while buf.len() < body_len {
        let mut tmp = vec![0u8; 1024];
        let n = stream.read(&mut tmp).map_err(|e| format!("read: {}", e))?;
        if n == 0 { return Err("connection closed".to_string()); }
        buf.extend_from_slice(&tmp[..n]);
    }
    let payload = buf[..body_len].to_vec();
    Ok((frame, payload))
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").expect("Failed to connect");
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();

    // STARTUP
    let startup = StartupMessage::new();
    let startup_payload = startup.encode_payload();
    let startup_frame = Frame::new(STARTUP, 0, startup_payload.len() as i32);
    let mut startup_msg = startup_frame.encode();
    startup_msg.put_slice(&startup_payload);
    stream.write_all(&startup_msg).unwrap();

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("STARTUP response: msg_type={} body_len={}", frame.msg_type, frame.body_len);
    hex_dump(&body, "  body");

    let startup_resp = StartupResponse::from_bytes(&body, frame.response_code).unwrap();
    println!("  version={} challenge_len={}", startup_resp.server_version, startup_resp.challenge.len());

    // LOGIN
    let login = LoginMessage::new("SYSDBA", "SYSDBA", "localhost");
    let login_payload = login.encode_payload(&startup_resp.challenge);
    let login_frame = Frame::new(LOGIN, 0, login_payload.len() as i32);
    let mut login_msg = login_frame.encode();
    login_msg.put_slice(&login_payload);
    stream.write_all(&login_msg).unwrap();

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("\nLOGIN response: msg_type={} body_len={}", frame.msg_type, frame.body_len);

    let login_resp = LoginResponse::from_bytes(&body).unwrap();
    println!("  server={} user={}", login_resp.server_name, login_resp.username);

    // READY
    let ready_payload: BytesMut = BytesMut::new();
    let ready_frame = Frame::new(READY, 0, 0);
    let ready_msg = ready_frame.encode();
    stream.write_all(&ready_msg).unwrap();

    let (frame, _body) = read_message(&mut stream).unwrap();
    println!("\nREADY response: msg_type={}", frame.msg_type);

    // EXEC: SELECT 1 FROM DUAL
    let sql = b"SELECT 1 FROM DUAL";
    let mut exec_payload = BytesMut::new();
    exec_payload.put_slice(sql);
    exec_payload.put_u8(0);
    let exec_frame = Frame::new(EXEC, 1, exec_payload.len() as i32);
    let mut exec_msg = exec_frame.encode();
    exec_msg.put_slice(&exec_payload);
    stream.write_all(&exec_msg).unwrap();
    println!("\nSent EXEC (handle=1)");

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("EXEC response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
    hex_dump(&body, "  body");

    // Try to parse as ExecResponse
    match ExecResponse::from_bytes(&body) {
        Ok(resp) => {
            println!("\nParsed ExecResponse:");
            println!("  col_count={}", resp.col_count);
            println!("  row_count={}", resp.row_count);
            println!("  columns={}", resp.columns.len());
            for (i, col) in resp.columns.iter().enumerate() {
                println!("  col[{}]: name={} type_code={} type_name={}", i, col.name, col.type_code, col.type_name);
            }
            println!("  rows={}", resp.rows.len());
            for (i, row) in resp.rows.iter().enumerate() {
                println!("  row[{}]: id={} values={:?}", i, row.row_id, row.values);
            }
        }
        Err(e) => println!("\nFailed to parse ExecResponse: {:?}", e),
    }

    // COMMIT
    let commit_payload: BytesMut = BytesMut::new();
    let commit_frame = Frame::new(COMMIT, 0, 0);
    let commit_msg = commit_frame.encode();
    stream.write_all(&commit_msg).unwrap();
    println!("\nSent COMMIT");

    let (frame, body) = read_message(&mut stream).unwrap();
    println!("COMMIT response: msg_type={} body_len={} resp_code={}", frame.msg_type, frame.body_len, frame.response_code);
    hex_dump(&body, "  body");
}
