use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn read_all(s: &mut TcpStream, timeout_ms: u64) -> Vec<u8> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if std::time::Instant::now() > deadline {
            break;
        }
        match s.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    buf
}

fn print_hex(label: &str, data: &[u8]) {
    print!("{} ({} bytes): ", label, data.len());
    for &b in data.iter().take(48) {
        print!("{:02x} ", b);
    }
    println!();
}

fn main() -> std::io::Result<()> {
    let mut s = TcpStream::connect("127.0.0.1:5236")?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    s.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Send READY
    let ready = [0u8; 64];
    // Actually let's just use the dameng protocol directly
    // First, let's read any pending data
    let startup = s.read(&mut [0u8; 4096]);
    println!("Startup read: {:?}", startup);

    // Write SYSDBA login - skip this, just use the rust library to connect and then debug the exec
    Ok(())
}
