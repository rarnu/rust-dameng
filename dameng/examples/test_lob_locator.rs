use dameng::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA")?;

    let _ = client.query("DROP TABLE IF EXISTS RUST_LOB_TEST");
    client.query(
        "CREATE TABLE RUST_LOB_TEST (
            ID INT PRIMARY KEY,
            BIG_CLOB CLOB,
            BIG_BLOB BLOB
        )",
    )?;

    // Test small inline CLOB
    client.query("INSERT INTO RUST_LOB_TEST (ID, BIG_CLOB) VALUES (1, 'HELLO')")?;

    // Test medium inline CLOB (~1000 bytes)
    let medium = "M".repeat(1000);
    let escaped = medium.replace('\'', "''");
    client.query(&format!(
        "INSERT INTO RUST_LOB_TEST (ID, BIG_CLOB) VALUES (2, '{}')",
        escaped
    ))?;

    // Test large out-of-row CLOB (3000 bytes)
    client.query("INSERT INTO RUST_LOB_TEST (ID) VALUES (3)")?;
    client.query("UPDATE RUST_LOB_TEST SET BIG_CLOB = RPAD('L', 3000, 'L') WHERE ID = 3")?;

    // Test large out-of-row BLOB
    client.query("INSERT INTO RUST_LOB_TEST (ID) VALUES (4)")?;
    client.query("UPDATE RUST_LOB_TEST SET BIG_BLOB = HEXTORAW(LPAD('AB', 4000, 'CD')) WHERE ID = 4")?;

    // Read back and verify using DmValue decoder
    let rs = client.query("SELECT ID, BIG_CLOB FROM RUST_LOB_TEST ORDER BY ID")?;
    println!("=== Inline LOB test results ===");
    for row in &rs.rows {
        let id_val = row.get(0, &rs.columns);
        let clob_val = row.get(1, &rs.columns);
        println!(
            "ID={:?}, BIG_CLOB={:?}",
            id_val,
            clob_val
                .as_ref()
                .map(|v| match v {
                    dameng_types::DmValue::Text(s) => format!("Text({} chars)", s.len()),
                    dameng_types::DmValue::LobLocator(l) => {
                        format!(
                            "LobLocator(blob_id={}, is_clob={})",
                            l.blob_id(),
                            l.is_clob
                        )
                    }
                    _ => format!("{:?}", v),
                })
                .unwrap_or("None".to_string())
        );
    }

    // Read BLOB
    let rs = client.query("SELECT ID, BIG_BLOB FROM RUST_LOB_TEST WHERE ID=4")?;
    for row in &rs.rows {
        let blob_val = row.get(1, &rs.columns);
        println!(
            "BLOB={:?}",
            blob_val
                .as_ref()
                .map(|v| match v {
                    dameng_types::DmValue::Bytea(b) => format!("Bytea({} bytes)", b.len()),
                    dameng_types::DmValue::LobLocator(l) => {
                        format!(
                            "LobLocator(blob_id={}, is_clob={})",
                            l.blob_id(),
                            l.is_clob
                        )
                    }
                    _ => format!("{:?}", v),
                })
                .unwrap_or("None".to_string())
        );
    }

    client.query("DROP TABLE RUST_LOB_TEST")?;
    println!("Done!");

    Ok(())
}
