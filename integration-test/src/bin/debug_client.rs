use std::env;
use dameng::Client;

fn main() {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = env::var("DM_PORT").unwrap_or_else(|_| "5236".to_string()).parse().unwrap_or(5236);
    let username = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".to_string());
    let password = env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".to_string());

    println!("Connecting to {}:{} ...", host, port);
    let mut client = Client::new(&host, port);
    client.connect(&username, &password).expect("Failed to connect");
    println!("Connected!");

    // Execute SELECT 1 FROM DUAL
    println!("\n=== SELECT 1 FROM DUAL ===");
    match client.execute("SELECT 1 FROM DUAL") {
        Ok(rows) => {
            println!("OK: Got {} row(s)", rows.len());
            for (i, row) in rows.iter().enumerate() {
                println!("  Row {}: values={:?}", i, row.values);
                if let Ok(val) = row.get_i32(0) {
                    println!("    col 0 (i32) = {}", val);
                }
            }
        }
        Err(e) => println!("FAIL: {}", e),
    }
}
