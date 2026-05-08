//! CRUD operations example with transactions.
//!
//! Usage: cargo run --example crud
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

    let _ = client.execute("DROP TABLE IF EXISTS EXAMPLE_USERS");

    println!("=== CREATE TABLE ===");
    client.execute("CREATE TABLE EXAMPLE_USERS (ID INT PRIMARY KEY, NAME VARCHAR(50), AGE INT)")?;
    println!("Table created.\n");

    println!("=== INSERT ===");
    client.execute("INSERT INTO EXAMPLE_USERS (ID, NAME, AGE) VALUES (1, 'Alice', 25)")?;
    client.execute("INSERT INTO EXAMPLE_USERS (ID, NAME, AGE) VALUES (2, 'Bob', 30)")?;
    client.execute("INSERT INTO EXAMPLE_USERS (ID, NAME, AGE) VALUES (3, 'Charlie', 35)")?;
    println!("Inserted 3 rows.\n");

    println!("=== SELECT ===");
    let rs = client.execute("SELECT ID, NAME, AGE FROM EXAMPLE_USERS ORDER BY ID")?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(0).ok().unwrap_or_default();
        let age = row.get_i32(1).ok().map(|v| format!("{}", v)).unwrap_or_default();
        println!("  ID={}, NAME={}, AGE={}", id, name, age);
    }

    println!("\n=== UPDATE ===");
    client.execute("UPDATE EXAMPLE_USERS SET AGE = 26 WHERE NAME = 'Alice'")?;
    let rs = client.execute("SELECT AGE FROM EXAMPLE_USERS WHERE NAME = 'Alice'")?;
    if let Some(row) = rs.first() {
        if let Ok(age) = row.get_i32(0) {
            println!("  Alice's new age: {}", age);
        }
    }

    println!("\n=== Transaction (ROLLBACK) ===");
    client.execute("INSERT INTO EXAMPLE_USERS (ID, NAME, AGE) VALUES (100, 'Temp', 99)")?;
    client.rollback()?;
    let rs = client.execute("SELECT COUNT(*) FROM EXAMPLE_USERS")?;
    if let Some(row) = rs.first() {
        if let Ok(count) = row.get_i32(0) {
            println!("  After rollback: {} rows", count);
        }
    }

    println!("\n=== Transaction (COMMIT) ===");
    client.execute("DELETE FROM EXAMPLE_USERS WHERE ID = 3")?;
    client.commit()?;
    let rs = client.execute("SELECT COUNT(*) FROM EXAMPLE_USERS")?;
    if let Some(row) = rs.first() {
        if let Ok(count) = row.get_i32(0) {
            println!("  After commit: {} rows", count);
        }
    }

    let _ = client.execute("DROP TABLE EXAMPLE_USERS");

    println!("\nDone!");
    Ok(())
}
