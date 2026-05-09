//! Data type coverage examples: INT, VARCHAR, TIMESTAMP, NULL handling, COUNT.
//!
//! Tests all column types across the three tables:
//!   - sample: INT (PK), VARCHAR(50)
//!   - sample_detail: INT (PK), VARCHAR(128) NOT NULL, VARCHAR(16) NOT NULL
//!   - sample_item: INT (PK part), INT (PK part), VARCHAR(32) NOT NULL, TIMESTAMP NOT NULL
//!
//! Usage: cargo run --package dameng --example data_types
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

    // Prepare
    println!("=== Preparing test data ===");
    let _ = client.execute("DELETE FROM sample_item WHERE sample_id IN (1, 2, 3)");
    let _ = client.execute("DELETE FROM sample_detail WHERE id IN (1, 2, 3)");
    let _ = client.execute("DELETE FROM sample WHERE id IN (1, 2, 3)");

    client.execute("INSERT INTO sample (ID, NAME) VALUES (1, 'Alice')")?;
    client.execute("INSERT INTO sample (ID, NAME) VALUES (2, 'Bob')")?;
    client.execute("INSERT INTO sample (ID, NAME) VALUES (3, 'Charlie')")?;

    client.execute(
        "INSERT INTO sample_detail (ID, ADDRESS, PHONE) VALUES (1, 'Beijing 123', '13800138000')",
    )?;
    client.execute(
        "INSERT INTO sample_detail (ID, ADDRESS, PHONE) VALUES (2, 'Shanghai 456', '13900139000')",
    )?;

    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (1, 101, 'Keyboard', '2024-01-15 10:30:00')",
    )?;
    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (1, 102, 'Mouse', '2024-02-20 14:45:00')",
    )?;
    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (2, 201, 'Monitor', '2024-03-10 09:00:00')",
    )?;
    println!("Data prepared.\n");

    // 1. INT type - column metadata + row value
    println!("=== INT type (sample.ID) ===");
    let rs = client.execute("SELECT ID FROM SAMPLE WHERE ID = 1")?;
    println!("  Column: {} (type_code={}, type_name={})",
        rs.columns[0].name, rs.columns[0].type_code, rs.columns[0].type_name);
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        println!("  Value: {}", id);
    }

    // 2. VARCHAR type
    println!("\n=== VARCHAR type (sample.NAME, sample_detail.ADDRESS) ===");
    let rs = client.execute("SELECT S.NAME, SD.ADDRESS FROM SAMPLE S JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID WHERE S.ID = 1")?;
    println!("  Columns: {:?}",
        rs.columns.iter().map(|c| format!("{}({},{})", c.name, c.type_code, c.type_name)).collect::<Vec<_>>());
    for row in rs.iter() {
        let name = row.get_str(0)?;
        let addr = row.get_str(1)?;
        println!("  NAME={}, ADDRESS={}", name, addr);
    }

    // 3. TIMESTAMP type - stored as string from protocol
    println!("\n=== TIMESTAMP type (sample_item.BUY_TIME) ===");
    let rs = client.execute("SELECT ITEM_NAME, BUY_TIME FROM SAMPLE_ITEM WHERE SAMPLE_ID = 1 ORDER BY BUY_TIME")?;
    println!("  Columns: {:?}",
        rs.columns.iter().map(|c| format!("{}({},{})", c.name, c.type_code, c.type_name)).collect::<Vec<_>>());
    for row in rs.iter() {
        let item = row.get_str(0)?;
        let time = row.get_str(1)?;
        println!("  ITEM_NAME={}, BUY_TIME={}", item, time);
    }

    // 4. NULL handling - LEFT JOIN produces NULLs
    println!("\n=== NULL handling (LEFT JOIN with missing detail) ===");
    let rs = client.execute(
        "SELECT S.ID, S.NAME, SD.ADDRESS, SD.PHONE \
         FROM SAMPLE S LEFT JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID \
         WHERE S.ID = 3",
    )?;
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(0)?;
        println!("  ID={}, NAME={}", id, name);
        println!("    ADDRESS is_null={}, PHONE is_null={}", row.is_null(1), row.is_null(2));
        let addr = if row.is_null(1) { "NULL".to_string() } else { row.get_str(1).ok().unwrap_or_default() };
        let phone = if row.is_null(2) { "NULL".to_string() } else { row.get_str(2).ok().unwrap_or_default() };
        println!("    ADDRESS={}, PHONE={}", addr, phone);
    }

    // 5. COUNT (aggregate) returns INT/BIGINT
    println!("\n=== COUNT aggregate ===");
    let rs = client.execute("SELECT COUNT(*) AS TOTAL FROM SAMPLE")?;
    println!("  Column: {} (type_code={}, type_name={})",
        rs.columns[0].name, rs.columns[0].type_code, rs.columns[0].type_name);
    for row in rs.iter() {
        let total = row.get_i32(0)?;
        println!("  COUNT(*) = {}", total);
    }

    // 6. Composite key (multi-column PK) - sample_item
    println!("\n=== Composite PK (sample_item: SAMPLE_ID + ITEM_ID) ===");
    let rs = client.execute("SELECT SAMPLE_ID, ITEM_ID, ITEM_NAME FROM SAMPLE_ITEM ORDER BY SAMPLE_ID, ITEM_ID")?;
    println!("  Columns: {:?}",
        rs.columns.iter().map(|c| format!("{}({})", c.name, c.type_name)).collect::<Vec<_>>());
    for row in rs.iter() {
        let sid = row.get_i32(0)?;
        let iid = row.get_i32(1)?;
        let name = row.get_str(2)?;
        println!("  SAMPLE_ID={}, ITEM_ID={}, ITEM_NAME={}", sid, iid, name);
    }

    // 7. Multi-column result with mixed types
    println!("\n=== Mixed types (INT + VARCHAR + TIMESTAMP) ===");
    let rs = client.execute(
        "SELECT SI.SAMPLE_ID, S.NAME, SI.ITEM_NAME, SI.BUY_TIME \
         FROM SAMPLE_ITEM SI JOIN SAMPLE S ON SI.SAMPLE_ID = S.ID \
         ORDER BY SI.SAMPLE_ID, SI.ITEM_ID",
    )?;
    println!("  Columns: {:?}",
        rs.columns.iter().map(|c| format!("{}({},{})", c.name, c.type_code, c.type_name)).collect::<Vec<_>>());
    for row in rs.iter() {
        let sid = row.get_i32(0)?;
        let name = row.get_str(1)?;
        let item = row.get_str(2)?;
        let time = row.get_str(3)?;
        println!("  SID={}, NAME={}, ITEM={}, TIME={}", sid, name, item, time);
    }

    // 8. Row value length
    println!("\n=== Row column count ===");
    let rs = client.execute("SELECT S.ID, S.NAME, SD.ADDRESS, SD.PHONE, SI.ITEM_NAME FROM SAMPLE S JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID JOIN SAMPLE_ITEM SI ON S.ID = SI.SAMPLE_ID")?;
    for row in rs.iter() {
        println!("  Row has {} columns", row.len());
        assert!(!row.is_empty());
    }

    // 9. Column nullable metadata
    println!("\n=== Column nullable metadata ===");
    let rs = client.execute("SELECT ID, ADDRESS, PHONE FROM SAMPLE_DETAIL")?;
    for col in rs.columns.iter() {
        println!("  {} : nullable={}", col.name, col.nullable);
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
