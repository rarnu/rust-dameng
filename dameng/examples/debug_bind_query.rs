use dameng::Client;
use dameng_protocol::message::bind::BindParam;
use dameng_protocol::message::bind::ParameterDirection;
use dameng_protocol::message::{BuildMessage, BindExec2Message, ExecMessage, EXEC, BIND_EXEC2, OPTIMIZED_PREPARE_EXEC, READY, EXEC_RESPONSE, ACK};
use dameng_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use bytes::BytesMut;
use std::io::{Read, Write};
use std::net::TcpStream;

fn build_message(msg_type: u8, stmt_id: u32, payload: &[u8]) -> Vec<u8> {
    let frame = Frame::new(msg_type, stmt_id, payload.len() as u64);
    let mut buf = frame.encode();
    buf.extend_from_slice(payload);
    buf
}

fn read_message(stream: &mut TcpStream) -> (Frame, Vec<u8>) {
    let mut buf = BytesMut::with_capacity(FRAME_HEADER_SIZE + 4096);
    
    loop {
        if buf.len() >= FRAME_HEADER_SIZE { break; }
        let mut tmp = vec![0u8; 1024];
        let n = stream.read(&mut tmp).unwrap();
        if n == 0 { panic!("connection closed"); }
        buf.extend_from_slice(&tmp[..n]);
    }
    
    let frame = Frame::parse(&mut buf).unwrap();
    let body_len = frame.body_len.max(0) as usize;
    while buf.len() < body_len {
        let mut tmp = vec![0u8; 1024];
        let n = stream.read(&mut tmp).unwrap();
        if n == 0 { panic!("connection closed"); }
        buf.extend_from_slice(&tmp[..n]);
    }
    
    let payload = buf[..body_len].to_vec();
    (frame, payload)
}

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5236").unwrap();
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    
    // Startup
    let startup_payload: Vec<u8> = vec![];
    let startup_frame = Frame::new(1, 0, 0); // STARTUP=1
    stream.write_all(&startup_frame.encode()).unwrap();
    startup_frame.encode().extend_from_slice(&startup_payload);
    
    // Just use Client for auth
    let mut c = Client::new("127.0.0.1", 5236);
    c.connect("SYSDBA", "SYSDBA").unwrap();
    
    // Create table
    c.execute("DROP TABLE IF EXISTS DM_DEBUG").unwrap();
    c.execute("CREATE TABLE DM_DEBUG (ID INT, NAME VARCHAR(50))").unwrap();
    c.execute("INSERT INTO DM_DEBUG VALUES (1, 'Alice')").unwrap();
    c.execute("INSERT INTO DM_DEBUG VALUES (2, 'Bob')").unwrap();
    
    eprintln!("\n=== Test OPE(91) for query ===");
    let rs = c.query("SELECT ID, NAME FROM DM_DEBUG").unwrap();
    eprintln!("  OPE query: rows={}, total={}", rs.rows.len(), rs.total_row_count);
    
    // Need fresh connection since c is consumed
    let mut c = Client::new("127.0.0.1", 5236);
    c.connect("SYSDBA", "SYSDBA").unwrap();
    
    eprintln!("\n=== Test EXEC(5)+BIND_EXEC2 for query ===");
    eprintln!("  (This is what do_prepare_execute does with params)");
    
    // Create a bind param for ?=0
    let params = vec![
        BindParam {
            type_name: "INT".to_string(),
            type_code: 4,
            precision: 0,
            scale: 0,
            direction: ParameterDirection::Input,
            value: Some(0i32.to_le_bytes().to_vec()),
        },
    ];
    
    let sql = "SELECT ID, NAME FROM DM_DEBUG WHERE ID > ?";
    let stmt_id = c.handle;
    
    // READY
    let ready_frame = Frame::new(READY, 0, 0);
    stream.write_all(&ready_frame.encode()).unwrap();
    
    // ... this is too complex, let's just add eprintln to client.rs
    eprintln!("  Let me add debug prints to do_prepare_execute instead");
    
    // Cleanup
    let _ = c.execute("DROP TABLE DM_DEBUG");
}
