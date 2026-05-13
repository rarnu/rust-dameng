//! JOIN query examples: LEFT JOIN, INNER JOIN, aggregation with subqueries.
//!
//! Uses the three test tables: sample, sample_detail, sample_item
//!
//! Usage: cargo run --package dameng --example join_queries
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

    client.execute("INSERT INTO sample (ID, NAME) VALUES (1, 'Alice')")?;
    client.execute("INSERT INTO sample (ID, NAME) VALUES (2, 'Bob')")?;
    client.execute("INSERT INTO sample (ID, NAME) VALUES (3, 'Charlie')")?;

    client.execute(
        "INSERT INTO sample_detail (ID, ADDRESS, PHONE) VALUES (1, 'Beijing 123', '13800138000')",
    )?;
    client.execute(
        "INSERT INTO sample_detail (ID, ADDRESS, PHONE) VALUES (2, 'Shanghai 456', '13900139000')",
    )?;
    // ID=3 has no sample_detail record (for LEFT JOIN NULL test)

    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (1, 101, 'Keyboard', '2024-01-15 10:30:00')",
    )?;
    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (1, 102, 'Mouse', '2024-02-20 14:45:00')",
    )?;
    client.execute(
        "INSERT INTO sample_item (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (2, 201, 'Monitor', '2024-03-10 09:00:00')",
    )?;
    println!("Test data prepared.\n");

    // LEFT JOIN: sample LEFT JOIN sample_detail
    println!("=== LEFT JOIN: sample + sample_detail ===");
    let rs = client.query(
        "SELECT S.ID, S.NAME, SD.ADDRESS, SD.PHONE FROM SAMPLE S LEFT JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID ORDER BY S.ID",
    )?;
    println!(
        "  Columns: {:?}",
        rs.columns
            .iter()
            .map(|c| format!("{}({})", c.name, c.type_name))
            .collect::<Vec<_>>()
    );
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        let addr = if row.is_null(2) {
            "NULL".to_string()
        } else {
            row.get_str(2)?.to_string()
        };
        let phone = if row.is_null(3) {
            "NULL".to_string()
        } else {
            row.get_str(3)?.to_string()
        };
        println!("  ID={}, NAME={}, ADDRESS={}, PHONE={}", id, name, addr, phone);
    }

    // LEFT JOIN with WHERE parameter: filter by sample.id
    println!("\n=== LEFT JOIN with WHERE id = 1 ===");
    let rs = client.query(
        "SELECT S.ID, S.NAME, SD.ADDRESS, SD.PHONE FROM SAMPLE S LEFT JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID WHERE S.ID = 1",
    )?;
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        let addr = row.get_str(2)?;
        let phone = row.get_str(3)?;
        println!("  ID={}, NAME={}, ADDRESS={}, PHONE={}", id, name, addr, phone);
    }

    // LEFT JOIN: sample LEFT JOIN sample_detail WHERE id = 3 (no detail record)
    println!("\n=== LEFT JOIN with WHERE id = 3 (missing detail) ===");
    let rs = client.query(
        "SELECT S.ID, S.NAME, SD.ADDRESS, SD.PHONE FROM SAMPLE S LEFT JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID WHERE S.ID = 3",
    )?;
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        let addr = if row.is_null(2) {
            "NULL".to_string()
        } else {
            row.get_str(2)?.to_string()
        };
        let phone = if row.is_null(3) {
            "NULL".to_string()
        } else {
            row.get_str(3)?.to_string()
        };
        println!("  ID={}, NAME={}, ADDRESS={}, PHONE={}", id, name, addr, phone);
    }

    // THREE-table JOIN: sample + sample_detail + sample_item
    println!("\n=== THREE-table JOIN: sample + sample_detail + sample_item ===");
    let rs = client.query(
        "SELECT S.ID, S.NAME, SD.ADDRESS, SI.ITEM_NAME, TO_CHAR(SI.BUY_TIME, 'YYYY-MM-DD HH24:MI:SS') AS BUY_TIME FROM SAMPLE S LEFT JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID LEFT JOIN SAMPLE_ITEM SI ON S.ID = SI.SAMPLE_ID ORDER BY S.ID, SI.ITEM_ID",
    )?;
    println!(
        "  Columns: {:?}",
        rs.columns
            .iter()
            .map(|c| format!("{}({})", c.name, c.type_name))
            .collect::<Vec<_>>()
    );
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        let addr = if row.is_null(2) {
            "NULL".to_string()
        } else {
            row.get_str(2)?.to_string()
        };
        let item = if row.is_null(3) {
            "NULL".to_string()
        } else {
            row.get_str(3)?.to_string()
        };
        let time = if row.is_null(4) {
            "NULL".to_string()
        } else {
            row.get_str(4)?.to_string()
        };
        println!(
            "  ID={}, NAME={}, ADDRESS={}, ITEM={}, BUY_TIME={}",
            id, name, addr, item, time
        );
    }

    // Aggregation: COUNT items per sample
    println!("\n=== Aggregation: COUNT items per sample ===");
    let rs = client.query(
        "SELECT S.ID, S.NAME, COUNT(SI.ITEM_ID) AS ITEM_COUNT FROM SAMPLE S LEFT JOIN SAMPLE_ITEM SI ON S.ID = SI.SAMPLE_ID GROUP BY S.ID, S.NAME ORDER BY S.ID",
    )?;
    println!(
        "  Columns: {:?}",
        rs.columns
            .iter()
            .map(|c| format!("{}({})", c.name, c.type_name))
            .collect::<Vec<_>>()
    );
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        let count = row.get_i32(2)?;
        println!("  ID={}, NAME={}, ITEM_COUNT={}", id, name, count);
    }

    // Subquery: samples with items bought after a certain date
    println!("\n=== Subquery: samples with recent purchases ===");
    let rs = client.query(
        "SELECT S.ID, S.NAME FROM SAMPLE S WHERE S.ID IN (SELECT DISTINCT SI.SAMPLE_ID FROM SAMPLE_ITEM SI WHERE SI.BUY_TIME >= '2024-02-01 00:00:00') ORDER BY S.ID",
    )?;
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        println!("  ID={}, NAME={}", id, name);
    }

    // EXISTS subquery: samples that have detail records
    println!("\n=== EXISTS: samples with detail records ===");
    let rs = client.query(
        "SELECT S.ID, S.NAME FROM SAMPLE S WHERE EXISTS (SELECT 1 FROM SAMPLE_DETAIL SD WHERE SD.ID = S.ID) ORDER BY S.ID",
    )?;
    for row in rs.iter() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        println!("  ID={}, NAME={}", id, name);
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
