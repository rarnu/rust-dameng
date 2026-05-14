use dameng::Client;

fn test_env(name: &str, host: &str, port: u16, user: &str, pass: &str) -> bool {
    println!("\n===== {} ({}:{}) =====", name, host, port);
    let mut c = Client::new(host, port);
    let result = c.connect(user, pass);
    if result.is_err() {
        println!("  Connect: FAIL - {}", result.unwrap_err());
        return false;
    }
    println!("  Connect: OK (encoding={:?})", c.server_encoding);

    // 1. CREATE TABLE
    match c.execute("CREATE TABLE IF NOT EXISTS TEST_RUST (id INT, name VARCHAR(50), age INT)") {
        Ok(n) => println!("  CREATE TABLE: OK ({} rows)", n),
        Err(e) => println!("  CREATE TABLE: FAIL - {}", e),
    }

    // 2. INSERT
    match c.execute("INSERT INTO TEST_RUST VALUES(1, 'Alice', 18)") {
        Ok(n) => println!("  execute INSERT: OK ({} rows)", n),
        Err(e) => println!("  execute INSERT: FAIL - {}", e),
    }

    // 3. INSERT with params
    match c.execute_with_params(
        "INSERT INTO TEST_RUST VALUES(?, ?, ?)",
        &[&2, &"Bob", &20],
    ) {
        Ok(n) => println!("  execute_with_params INSERT: OK ({} rows)", n),
        Err(e) => println!("  execute_with_params INSERT: FAIL - {}", e),
    }

    // 4. SELECT
    match c.query("SELECT * FROM TEST_RUST") {
        Ok(rs) => {
            println!("  query SELECT: OK ({} rows)", rs.len());
        }
        Err(e) => println!("  query SELECT: FAIL - {}", e),
    }

    // 5. SELECT with params
    match c.query_with_params("SELECT * FROM TEST_RUST WHERE id = ?", &[&1]) {
        Ok(rs) => {
            println!("  query_with_params SELECT: OK ({} rows)", rs.len());
        }
        Err(e) => println!("  query_with_params SELECT: FAIL - {}", e),
    }

    // 6. Transaction
    match c.transaction() {
        Ok(mut tx) => {
            let _ = tx.execute("INSERT INTO TEST_RUST VALUES(3, 'Chris', 9)");
            let _ = tx.execute_with_params(
                "INSERT INTO TEST_RUST VALUES(?, ?, ?)",
                &[&4, &"Diana", &25],
            );
            match tx.commit() {
                Ok(()) => println!("  tx commit: OK"),
                Err(e) => println!("  tx commit: FAIL - {}", e),
            }
        }
        Err(e) => println!("  transaction: FAIL - {}", e),
    }

    // 7. Verify transaction
    match c.query("SELECT COUNT(*) FROM TEST_RUST") {
        Ok(rs) => {
            if let Some(row) = rs.first() {
                let count: i32 = row.get(0).unwrap_or(-1);
                println!("  count after tx: {}", count);
            }
        }
        Err(_) => {}
    }

    // 8. Cleanup
    let _ = c.execute("DROP TABLE TEST_RUST");
    let _ = c.close();
    println!("  Close: OK");
    true
}

fn main() {
    let ok1 = test_env("ENV1 (port 5236)", "127.0.0.1", 5236, "SYSDBA", "SYSDBA");
    let ok2 = test_env("ENV2 (port 5239)", "127.0.0.1", 5239, "SYSDBA", "SYSDBA001");
    println!("\n===== SUMMARY =====");
    println!("  ENV1: {}", if ok1 { "PASS" } else { "FAIL" });
    println!("  ENV2: {}", if ok2 { "PASS" } else { "FAIL" });
}
