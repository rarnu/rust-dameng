//! Real parameter binding examples using execute_with_params.
//!
//! Demonstrates INT, VARCHAR, TIMESTAMP parameter binding with
//! INSERT, UPDATE, SELECT, DELETE via the actual bind parameter API.
//!
//! Usage: cargo run --package dameng --example real_param_binding
//! Environment: DM_HOST=127.0.0.1 DM_PORT=5236 DM_USER=SYSDBA DM_PASS=SYSDBA

use std::env;

use dameng::{BindParam, Client, ParameterDirection};
use dameng_types::{encode_value, DmValue, DmValueType};

fn make_int_param(value: i32) -> BindParam {
    BindParam {
        type_name: "INT".to_string(),
        type_code: 4,
        precision: 0,
        scale: 0,
        direction: ParameterDirection::Input,
        value: Some(encode_value(DmValueType::INT, &DmValue::Int(value))),
    }
}

fn make_varchar_param(value: &str) -> BindParam {
    let bytes = value.as_bytes();
    BindParam {
        type_name: "VARCHAR".to_string(),
        type_code: 3,
        precision: bytes.len() as i32,
        scale: 0,
        direction: ParameterDirection::Input,
        value: Some(encode_value(DmValueType::VARCHAR, &DmValue::Text(value.to_string()))),
    }
}

fn make_timestamp_param(value: &str) -> BindParam {
    let bytes = value.as_bytes();
    BindParam {
        type_name: "TIMESTAMP".to_string(),
        type_code: 12,
        precision: bytes.len() as i32,
        scale: 0,
        direction: ParameterDirection::Input,
        value: Some(encode_value(DmValueType::TIMESTAMP, &DmValue::Text(value.to_string()))),
    }
}

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
    let sql = "INSERT INTO SAMPLE (ID, NAME) VALUES (?, ?)";
    let params = vec![make_int_param(10), make_varchar_param("ParamAlice")];
    client.execute_with_params(0, sql, &params)?;
    println!("  Inserted: ID=10, NAME='ParamAlice'");

    let sql = "INSERT INTO SAMPLE (ID, NAME) VALUES (?, ?)";
    let params = vec![make_int_param(11), make_varchar_param("ParamBob")];
    client.execute_with_params(0, sql, &params)?;
    println!("  Inserted: ID=11, NAME='ParamBob'\n");

    // === SELECT with INT param ===
    println!("=== SELECT with INT param (WHERE ID = ?) ===");
    let sql = "SELECT ID, NAME FROM SAMPLE WHERE ID = ?";
    let params = vec![make_int_param(10)];
    let rs = client.execute_with_params(0, sql, &params)?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    // === SELECT with VARCHAR param ===
    println!("\n=== SELECT with VARCHAR param (WHERE NAME = ?) ===");
    let sql = "SELECT ID, NAME FROM SAMPLE WHERE NAME = ?";
    let params = vec![make_varchar_param("ParamBob")];
    let rs = client.execute_with_params(0, sql, &params)?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        println!("  ID={}, NAME={}", id, name);
    }

    // === UPDATE with multiple params ===
    println!("\n=== UPDATE with INT + VARCHAR params ===");
    let sql = "UPDATE SAMPLE SET NAME = ? WHERE ID = ?";
    let params = vec![make_varchar_param("UpdatedAlice"), make_int_param(10)];
    client.execute_with_params(0, sql, &params)?;
    println!("  Updated: NAME='UpdatedAlice' WHERE ID=10");

    // Verify update
    let sql = "SELECT ID, NAME FROM SAMPLE WHERE ID = ?";
    let params = vec![make_int_param(10)];
    let rs = client.execute_with_params(0, sql, &params)?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        println!("  Verified: ID={}, NAME={}", id, name);
    }

    // === INSERT sample_detail with VARCHAR params ===
    println!("\n=== INSERT sample_detail with VARCHAR params ===");
    let sql = "INSERT INTO SAMPLE_DETAIL (ID, ADDRESS, PHONE) VALUES (?, ?, ?)";
    let params = vec![
        make_int_param(10),
        make_varchar_param("Param Address 123"),
        make_varchar_param("13812345678"),
    ];
    client.execute_with_params(0, sql, &params)?;
    println!("  Inserted: ID=10, ADDRESS='Param Address 123', PHONE='13812345678'");

    // === INSERT sample_item with TIMESTAMP param ===
    println!("\n=== INSERT sample_item with INT + VARCHAR + TIMESTAMP params ===");
    let sql = "INSERT INTO SAMPLE_ITEM (SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME) VALUES (?, ?, ?, ?)";
    let params = vec![
        make_int_param(10),
        make_int_param(1001),
        make_varchar_param("Keyboard"),
        make_timestamp_param("2024-06-15 10:30:00"),
    ];
    client.execute_with_params(0, sql, &params)?;
    println!("  Inserted: SAMPLE_ID=10, ITEM_ID=1001, ITEM_NAME='Keyboard', BUY_TIME='2024-06-15 10:30:00'");

    // === SELECT with TIMESTAMP param (WHERE buy_time >= ?) ===
    println!("\n=== SELECT with TIMESTAMP param (WHERE BUY_TIME >= ?) ===");
    let sql = "SELECT SAMPLE_ID, ITEM_ID, ITEM_NAME, BUY_TIME FROM SAMPLE_ITEM WHERE BUY_TIME >= ?";
    let params = vec![make_timestamp_param("2024-06-01 00:00:00")];
    let rs = client.execute_with_params(0, sql, &params)?;
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
        println!("  SAMPLE_ID={}, ITEM_ID={}, ITEM_NAME={}, BUY_TIME={}", sid, iid, name, time);
    }

    // === DELETE with param ===
    println!("\n=== DELETE with INT param ===");
    let sql = "DELETE FROM SAMPLE_ITEM WHERE SAMPLE_ID = ?";
    let params = vec![make_int_param(10)];
    client.execute_with_params(0, sql, &params)?;
    println!("  Deleted: SAMPLE_ID=10");

    // Verify deletion
    let rs = client.query("SELECT COUNT(*) FROM SAMPLE_ITEM WHERE SAMPLE_ID = 10")?;
    for row in rs.iter() {
        let count = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        println!("  Remaining: {}", count);
    }

    // === LEFT JOIN with param ===
    println!("\n=== LEFT JOIN with param ===");
    let sql = "SELECT S.ID, S.NAME, SD.ADDRESS, SD.PHONE \
               FROM SAMPLE S LEFT JOIN SAMPLE_DETAIL SD ON S.ID = SD.ID \
               WHERE S.ID = ?";
    let params = vec![make_int_param(10)];
    let rs = client.execute_with_params(0, sql, &params)?;
    for row in rs.iter() {
        let id = row.get_i32(0).ok().map(|v| format!("{}", v)).unwrap_or_default();
        let name = row.get_str(1).ok().unwrap_or_default();
        let addr = if row.is_null(2) {
            String::from("NULL")
        } else {
            row.get_str(2).ok().unwrap_or_default()
        };
        let phone = if row.is_null(3) {
            String::from("NULL")
        } else {
            row.get_str(3).ok().unwrap_or_default()
        };
        println!("  ID={}, NAME={}, ADDRESS={}, PHONE={}", id, name, addr, phone);
    }

    // Cleanup
    println!("\n=== Cleanup ===");
    let _ = client.execute("DELETE FROM sample_item WHERE sample_id IN (10, 11)");
    let _ = client.execute("DELETE FROM sample_detail WHERE id IN (10, 11)");
    let _ = client.execute("DELETE FROM sample WHERE id IN (10, 11)");
    client.commit()?;

    println!("\nDone!");
    Ok(())
}
