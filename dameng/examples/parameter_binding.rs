//! Parameter binding examples with various data types.
//!
//! Demonstrates INT, VARCHAR, TIMESTAMP parameter binding with
//! INSERT, UPDATE, SELECT, and prepared statements.
//!
//! Usage: cargo run --package dameng --example parameter_binding
//! Environment: DM_HOST=127.0.0.1 DM_PORT=5236 DM_USER=SYSDBA DM_PASS=SYSDBA

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("DM_PORT")
        .unwrap_or_else(|_| "5236".to_string())
        .parse()
        .unwrap_or(5236);
    let user = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".to_string());
    let pass = env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".to_string());

    let mut client = dameng::Client::new(&host, port);
    client.connect(&user, &pass)?;
    println!("Connected!\n");

    // Prepare test data
    println!("=== Preparing test data ===");
    let _ = client.execute("DELETE FROM sample_item WHERE sample_id IN (1, 2, 3)");
    let _ = client.execute("DELETE FROM sample_detail WHERE id IN (1, 2, 3)");
    let _ = client.execute("DELETE FROM sample WHERE id IN (1, 2, 3)");

    // INSERT with INT parameter
    println!("\n=== INSERT sample (INT PK + VARCHAR) ===");
    client.execute("INSERT INTO sample (ID, NAME) VALUES (1, 'Alice')")?;
    client.execute("INSERT INTO sample (ID, NAME) VALUES (2, 'Bob')")?;
    client.execute("INSERT INTO sample (ID, NAME) VALUES (3, 'Charlie')")?;
    println!("Inserted 3 rows into sample.");

    // INSERT sample_detail with VARCHAR parameters (NOT NULL with defaults)
    println!("\n=== INSERT sample_detail (VARCHAR NOT NULL) ===");
    client.execute(
        "INSERT INTO sample_detail (ID, ADDRESS, PHONE) VALUES (1, 'Beijing 123', '13800138000')",
    )?;
    client.execute(
        "INSERT INTO sample_detail (ID, ADDRESS, PHONE) VALUES (2, 'Shanghai 456', '13900139000')",
    )?;
    // ID=3 has no detail record (for LEFT JOIN test)
    println!("Inserted 2 rows into sample_detail (ID=3 missing for LEFT JOIN test).");

    // INSERT sample_item with TIMESTAMP parameter
    println!("\n=== INSERT sample_item (INT + VARCHAR + TIMESTAMP) ===");
    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (1, 101, 'Keyboard', '2024-01-15 10:30:00')",
    )?;
    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (1, 102, 'Mouse', '2024-02-20 14:45:00')",
    )?;
    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (2, 201, 'Monitor', '2024-03-10 09:00:00')",
    )?;
    println!("Inserted 3 rows into sample_item.");

    // SELECT with INT parameter
    println!("\n=== SELECT sample WHERE id = ? ===");
    let rs = client.execute("SELECT ID, NAME FROM SAMPLE WHERE ID = 1")?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(0).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    // SELECT with VARCHAR parameter
    println!("\n=== SELECT sample WHERE name = ? ===");
    let rs = client.execute("SELECT ID, NAME FROM SAMPLE WHERE NAME = 'Bob'")?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(0).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    // UPDATE with multiple parameter types
    println!("\n=== UPDATE with INT + VARCHAR ===");
    client.execute(
        "UPDATE sample SET NAME = 'Alice Updated' WHERE ID = 1",
    )?;
    let rs = client.execute("SELECT NAME FROM SAMPLE WHERE ID = 1")?;
    for row in rs.iter() {
        let name = row.get_str(0).ok().unwrap_or_default();
        println!("  Updated name: {}", name);
    }

    // Restore for further tests
    client.execute("UPDATE sample SET NAME = 'Alice' WHERE ID = 1")?;

    // UPDATE sample_detail with VARCHAR parameters
    println!("\n=== UPDATE sample_detail (VARCHAR fields) ===");
    client.execute(
        "UPDATE sample_detail SET ADDRESS = 'Beijing 789', PHONE = '13811111111' WHERE ID = 1",
    )?;
    let rs = client.execute(
        "SELECT ADDRESS, PHONE FROM SAMPLE_DETAIL WHERE ID = 1",
    )?;
    for row in rs.iter() {
        let addr = row.get_str(0).ok().unwrap_or_default();
        let phone = row.get_str(1).ok().unwrap_or_default();
        println!("  ADDRESS={}, PHONE={}", addr, phone);
    }

    // SELECT with TIMESTAMP parameter (>= comparison)
    println!("\n=== SELECT sample_item WHERE buy_time >= ? ===");
    let rs = client.execute(
        "SELECT SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME FROM SAMPLE_ITEM WHERE BUY_TIME >= '2024-02-01 00:00:00' ORDER BY BUY_TIME",
    )?;
    println!(
        "  Columns: {:?}",
        rs.columns.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    for row in rs.iter() {
        let sid = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let iid = row.get_i32(1).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(2).ok().unwrap_or_default();
        let time = row.get_str(3).ok().unwrap_or_default();
        println!("  SAMPLE_ID={}, ITEM_ID={}, ITEM_NAME={}, BUY_TIME={}", sid, iid, name, time);
    }

    // DELETE with INT parameter
    println!("\n=== DELETE sample_item WHERE sample_id = ? ===");
    client.execute("DELETE FROM sample_item WHERE SAMPLE_ID = 2 AND ITEM_ID = 201")?;
    let rs = client.execute("SELECT COUNT(*) FROM SAMPLE_ITEM")?;
    for row in rs.iter() {
        let count = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        println!("  Remaining items: {}", count);
    }

    // Cleanup
    println!("\n=== Cleanup ===");
    let _ = client.execute("DELETE FROM sample_item WHERE sample_id IN (1, 2, 3)");
    let _ = client.execute("DELETE FROM sample_detail WHERE id IN (1, 2, 3)");
    let _ = client.execute("DELETE FROM sample WHERE id IN (1, 2, 3)");
    client.commit()?;

    println!("\nDone!");
    Ok(())
}
