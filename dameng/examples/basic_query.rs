//! Basic sync query example - connects to DM and runs SELECT queries.
//!
//! Usage: cargo run --example basic_query
//! Environment: DM_HOST=127.0.0.1 DM_PORT=5236 DM_USER=SYSDBA DM_PASS=SYSDBA

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("DM_PORT").unwrap_or_else(|_| "5236".to_string()).parse().unwrap_or(5236);
    let user = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".to_string());
    let pass = env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".to_string());

    println!("Connecting to Dameng {}:{} ...", host, port);
    let mut client = dameng::Client::new(&host, port);
    client.connect(&user, &pass)?;
    println!("Connected!\n");

    // SELECT 1
    println!("=== SELECT 1 ===");
    let rs = client.query("SELECT 1")?;
    for row in rs.iter() {
        if let Ok(val) = row.get_i32(0) {
            println!("  Result: {}", val);
        }
    }

    // SELECT from V$VERSION
    println!("\n=== V$VERSION ===");
    let rs = client.query("SELECT * FROM V$VERSION")?;
    for row in rs.iter() {
        if let Ok(ver) = row.get_str(0) {
            println!("  {}", ver);
        }
    }

    // SELECT from SAMPLE table
    println!("\n=== SELECT FROM SAMPLE ===");
    let rs = client.query("SELECT ID, NAME FROM SAMPLE ORDER BY ID")?;
    println!("  Columns: {:?}", rs.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    println!("\nDone!");
    Ok(())
}
