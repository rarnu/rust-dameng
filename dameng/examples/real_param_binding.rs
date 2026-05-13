//! Real parameter binding examples using execute_with_params / query_with_params.
//!
//! Demonstrates INT, VARCHAR, TIMESTAMP parameter binding with
//! INSERT, UPDATE, SELECT, DELETE via the actual bind parameter API.
//!
//! Usage: cargo run --package dameng --example real_param_binding
//! Environment: DM_HOST=127.0.0.1 DM_PORT=5236 DM_USER=SYSDBA DM_PASS=SYSDBA

use std::env;

use dameng::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("DM_PORT")
        .unwrap_or_else(|_| "5236".to_string())
        .parse()
        .unwrap_or(5236);
    let user = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".to_string());
    let pass = env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".to_string());

    let mut client = Client::new(&host, port);
    client.connect(&user, &pass)?;
    println!("Connected!\n");

    // Clean up test data
    let _ = client.execute("DELETE FROM sample_item WHERE sample_id IN (10, 11)");
    let _ = client.execute("DELETE FROM sample_detail WHERE id IN (10, 11)");
    let _ = client.execute("DELETE FROM sample WHERE id IN (10, 11)");

    // === INSERT with INT + VARCHAR params ===
    println!("=== INSERT with INT + VARCHAR params ===");
    let id1: i32 = 10;
    let name1 = String::from("ParamAlice");
    client.execute_with_params(
        "INSERT INTO SAMPLE (ID, NAME) VALUES (?, ?)",
        &[&id1, &name1],
    )?;
    println!("  Inserted: ID=10, NAME='ParamAlice'");

    let id2: i32 = 11;
    let name2 = String::from("ParamBob");
    client.execute_with_params(
        "INSERT INTO SAMPLE (ID, NAME) VALUES (?, ?)",
        &[&id2, &name2],
    )?;
    println!("  Inserted: ID=11, NAME='ParamBob'\n");

    // === SELECT with INT param ===
    println!("=== SELECT with INT param (WHERE ID = ?) ===");
    let find_id: i32 = 10;
    let rs = client.query_with_params("SELECT ID, NAME FROM SAMPLE WHERE ID = ?", &[&find_id])?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    // === SELECT with VARCHAR param ===
    println!("\n=== SELECT with VARCHAR param (WHERE NAME = ?) ===");
    let find_name = String::from("ParamBob");
    let rs =
        client.query_with_params("SELECT ID, NAME FROM SAMPLE WHERE NAME = ?", &[&find_name])?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    // === UPDATE with multiple params ===
    println!("\n=== UPDATE with INT + VARCHAR params ===");
    let updated_name = String::from("UpdatedAlice");
    let update_id: i32 = 10;
    client.execute_with_params(
        "UPDATE SAMPLE SET NAME = ? WHERE ID = ?",
        &[&updated_name, &update_id],
    )?;
    println!("  Updated: NAME='UpdatedAlice' WHERE ID=10");

    // Verify update
    let verify_id: i32 = 10;
    let rs =
        client.query_with_params("SELECT ID, NAME FROM SAMPLE WHERE ID = ?", &[&verify_id])?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        println!("  Verified: ID={}, NAME={}", id, name);
    }

    // === INSERT sample_detail with VARCHAR params ===
    println!("\n=== INSERT sample_detail with VARCHAR params ===");
    let detail_id: i32 = 10;
    let address = String::from("Param Address 123");
    let phone = String::from("13812345678");
    client.execute_with_params(
        "INSERT INTO SAMPLE_DETAIL (ID, ADDRESS, PHONE) VALUES (?, ?, ?)",
        &[&detail_id, &address, &phone],
    )?;
    println!("  Inserted: ID=10, ADDRESS='Param Address 123', PHONE='13812345678'");

    // === INSERT sample_item with TIMESTAMP param ===
    println!(
        "\n=== INSERT sample_item with INT + VARCHAR + TIMESTAMP params ==="
    );
    let sample_id: i32 = 10;
    let item_id: i32 = 1001;
    let item_name = String::from("Keyboard");
    let buy_time = String::from("2024-06-15 10:30:00");
    client.execute_with_params(
        "INSERT INTO SAMPLE_ITEM (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (?, ?, ?, ?)",
        &[&sample_id, &item_id, &item_name, &buy_time],
    )?;
    println!(
        "  Inserted: SAMPLE_ID=10, ITEM_ID=1001, ITEM_NAME='Keyboard', BUY_TIME='2024-06-15 10:30:00'"
    );

    // === SELECT with TIMESTAMP param (WHERE buy_time >= ?) ===
    println!("\n=== SELECT with TIMESTAMP param (WHERE BUY_TIME >= ?) ===");
    let time_filter = String::from("2024-06-01 00:00:00");
    let rs = client.query_with_params(
        "SELECT SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME FROM SAMPLE_ITEM WHERE BUY_TIME >= ?",
        &[&time_filter],
    )?;
    println!(
        "  Columns: {:?}",
        rs.columns
            .iter()
            .map(|c| format!("{}({})", c.name, c.type_name))
            .collect::<Vec<_>>()
    );
    for row in rs.iter() {
        let sid = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let iid = row.get_i32(1).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(2).ok().unwrap_or_default();
        let time = row.get_timestamp(3).ok().unwrap_or_default();
        println!(
            "  SAMPLE_ID={}, ITEM_ID={}, ITEM_NAME={}, BUY_TIME={}",
            sid, iid, name, time
        );
    }

    // === DELETE with param ===
    println!("\n=== DELETE with INT param ===");
    let del_id: i32 = 10;
    client.execute_with_params(
        "DELETE FROM SAMPLE_ITEM WHERE SAMPLE_ID = ?",
        &[&del_id],
    )?;
    println!("  Deleted: SAMPLE_ID=10");

    // Verify deletion
    let rs = client.query("SELECT COUNT(*) FROM SAMPLE_ITEM WHERE SAMPLE_ID = 10")?;
    for row in rs.iter() {
        let count = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        println!("  Remaining: {}", count);
    }

    // === LEFT JOIN with param ===
    println!("\n=== LEFT JOIN with param ===");
    let join_id: i32 = 10;
    let sql = "SELECT S.ID, S.NAME, SD.ADDRESS, SD.PHONE \
               FROM SAMPLE S LEFT JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID \
               WHERE S.ID = ?";
    let rs = client.query_with_params(sql, &[&join_id])?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        let addr = if row.is_null(2) {
            String::from("NULL")
        } else {
            row.get_str(2).ok().unwrap_or_default().to_string()
        };
        let phone = if row.is_null(3) {
            String::from("NULL")
        } else {
            row.get_str(3).ok().unwrap_or_default().to_string()
        };
        println!(
            "  ID={}, NAME={}, ADDRESS={}, PHONE={}",
            id, name, addr, phone
        );
    }

    // Cleanup
    println!("\n=== Cleanup ===");
    let _ = client.execute("DELETE FROM sample_item WHERE sample_id IN (10, 11)");
    let _ = client.execute("DELETE FROM sample_detail WHERE id IN (10, 11)");
    let _ = client.execute("DELETE FROM sample WHERE id IN (10, 11)");

    println!("\nDone!");
    Ok(())
}
