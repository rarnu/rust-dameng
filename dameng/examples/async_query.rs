//! Async query example using tokio-dameng.
//!
//! Usage: cargo run --example async_query
//! Environment: DM_HOST=127.0.0.1 DM_PORT=5236 DM_USER=SYSDBA DM_PASS=SYSDBA

use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("DM_PORT").unwrap_or_else(|_| "5236".to_string()).parse().unwrap_or(5236);
    let user = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".to_string());
    let pass = env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".to_string());

    println!("Connecting to Dameng {}:{} (async) ...", host, port);
    let mut client = tokio_dameng::Client::new(&host, port);
    client.connect(&user, &pass).await?;
    println!("Connected!\n");

    println!("=== SELECT 1 ===");
    let rs = client.execute("SELECT 1").await?;
    for row in &rs.rows {
        if let Ok(val) = row.get_i32(0) {
            println!("  Result: {}", val);
        }
    }

    println!("\n=== V$VERSION ===");
    let rs = client.execute("SELECT * FROM V$VERSION").await?;
    for row in &rs.rows {
        if let Ok(ver) = row.get_str(0) {
            println!("  {}", ver);
        }
    }

    println!("\n=== Query API ===");
    use tokio_dameng::QueryBuilderExt;
    let rs = client.query("SELECT 42 AS ANSWER").fetch_all().await?;
    for row in &rs.rows {
        if let Ok(val) = row.get_i32(0) {
            println!("  ANSWER: {}", val);
        }
    }

    println!("\n=== SELECT FROM SAMPLE ===");
    let rs = client.execute("SELECT ID, NAME FROM SAMPLE ORDER BY ID").await?;
    println!("  Columns: {:?}", rs.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
    for row in &rs.rows {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(0).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    println!("\nDone!");
    Ok(())
}
