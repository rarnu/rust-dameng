use dameng::Client;

/// Debug LOB data retrieval from DM server
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA")?;
    println!("Connected!\n");

    // Create test table
    let _ = client.query("DROP TABLE IF EXISTS RUST_LOB_TEST");
    client.query(
        "CREATE TABLE RUST_LOB_TEST (
            ID INT PRIMARY KEY,
            BIG_CLOB CLOB,
            BIG_BLOB BLOB
        )",
    )?;
    println!("Created table RUST_LOB_TEST\n");

    // Test 1: Insert empty, then check what empty CLOB looks like
    client.query("INSERT INTO RUST_LOB_TEST (ID, BIG_CLOB, BIG_BLOB) VALUES (1, '', X'')")?;
    println!("Test 1: Empty CLOB/BLOB");
    let rs = client.query("SELECT ID, BIG_CLOB, BIG_BLOB FROM RUST_LOB_TEST WHERE ID=1")?;
    for col in &rs.columns {
        println!("  col_name={} type_code={} type_name={}", col.name, col.type_code, col.type_name);
    }
    for row in &rs.rows {
        for (i, col) in rs.columns.iter().enumerate() {
            if let Some(Some(val)) = row.values.get(i) {
                println!("  {} ({} bytes): {:?}, is_lob_locator={}", 
                    col.name, val.len(), String::from_utf8_lossy(val), val.len() == 16);
            } else {
                println!("  {}: NULL", col.name);
            }
        }
    }
    println!();

    // Test 2: Insert medium CLOB (< 2048 bytes)
    let medium_clob = "M".repeat(1000);
    let escaped = medium_clob.replace('\'', "''");
    client.query(&format!(
        "INSERT INTO RUST_LOB_TEST (ID, BIG_CLOB) VALUES (2, '{}')",
        escaped
    ))?;
    println!("Test 2: Medium CLOB ({} bytes)", medium_clob.len());
    let rs = client.query("SELECT ID, BIG_CLOB FROM RUST_LOB_TEST WHERE ID=2")?;
    for row in &rs.rows {
        if let Some(Some(val)) = row.values.get(1) {
            println!("  BIG_CLOB value ({} bytes), is_lob_locator={}", val.len(), val.len() == 16);
            if val.len() == 16 {
                let hex: String = val.iter().map(|b| format!("{:02X}", b)).collect();
                println!("  LOB_LOCATOR hex: {}", hex);
            } else {
                let preview: String = String::from_utf8_lossy(val).chars().take(30).collect();
                println!("  Content preview: {:?}", preview);
            }
        }
    }
    println!();

    // Test 3: Use DM function to create large CLOB
    // DBMS_LOB.SUBSTR or other LOB functions
    client.query("INSERT INTO RUST_LOB_TEST (ID) VALUES (3)")?;
    
    // Try using RPAD to generate large CLOB
    client.query(
        "UPDATE RUST_LOB_TEST SET BIG_CLOB = RPAD('L', 3000, 'L') WHERE ID = 3"
    )?;
    println!("Test 3: Large CLOB via RPAD (3000 bytes)");
    let rs = client.query("SELECT ID, BIG_CLOB FROM RUST_LOB_TEST WHERE ID=3")?;
    for row in &rs.rows {
        if let Some(Some(val)) = row.values.get(1) {
            println!("  BIG_CLOB value ({} bytes), is_lob_locator={}", val.len(), val.len() == 16);
            if val.len() == 16 {
                let hex: String = val.iter().map(|b| format!("{:02X}", b)).collect();
                println!("  LOB_LOCATOR hex: {}", hex);
                // Parse the locator structure
                if val.len() == 16 {
                    let blob_id = i64::from_le_bytes(val[0..8].try_into().unwrap());
                    let group_id = i32::from_le_bytes(val[8..12].try_into().unwrap());
                    let file_id = i32::from_le_bytes(val[12..16].try_into().unwrap());
                    println!("  Parsed: blob_id={}, group_id={}, file_id={}", blob_id, group_id, file_id);
                }
            } else {
                let preview: String = String::from_utf8_lossy(val).chars().take(30).collect();
                println!("  Content preview: {:?}", preview);
            }
        }
    }
    println!();

    // Test 4: Large BLOB via DBMS_LOB or hex literal
    client.query("INSERT INTO RUST_LOB_TEST (ID) VALUES (4)")?;
    client.query(
        "UPDATE RUST_LOB_TEST SET BIG_BLOB = HEXTORAW('DEADBEEF' || LPAD('0', 5000, '0')) WHERE ID = 4"
    )?;
    println!("Test 4: Large BLOB via HEXTORAW");
    let rs = client.query("SELECT ID, BIG_BLOB FROM RUST_LOB_TEST WHERE ID=4")?;
    for row in &rs.rows {
        if let Some(Some(val)) = row.values.get(1) {
            println!("  BIG_BLOB value ({} bytes), is_lob_locator={}", val.len(), val.len() == 16);
            if val.len() == 16 {
                let hex: String = val.iter().map(|b| format!("{:02X}", b)).collect();
                println!("  LOB_LOCATOR hex: {}", hex);
            }
        }
    }
    println!();

    // Cleanup
    client.query("DROP TABLE RUST_LOB_TEST")?;
    println!("Cleanup done!");

    Ok(())
}
