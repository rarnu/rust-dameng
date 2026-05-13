use dameng::Client;

fn main() {
    let mut c = Client::new("127.0.0.1", 5236).unwrap();
    c.connect("SYSDBA", "SYSDBA").unwrap();

    eprintln!("=== Test OPE(91) for SELECT 1 (original query path) ===");
    // Use the internal method directly to test
    match c.query("SELECT 1") {
        Ok(rs) => eprintln!("  query(SELECT 1): rows={}, total={}", rs.rows.len(), rs.total_row_count),
        Err(e) => eprintln!("  query(SELECT 1): ERROR: {}", e),
    }

    // Try to reconnect
    let mut c2 = Client::new("127.0.0.1", 5236).unwrap();
    c2.connect("SYSDBA", "SYSDBA").unwrap();
    
    eprintln!("=== Test execute with simple DML ===");
    match c2.execute("INSERT INTO SAMPLE (SAMPLE_ID, ITEM_ID, ITEM_NAME) VALUES (999, 'X', 'DEBUG_TEST')") {
        Ok(n) => eprintln!("  execute(INSERT): affected={}", n),
        Err(e) => eprintln!("  execute(INSERT): ERROR: {}", e),
    }

    match c2.execute("DELETE FROM SAMPLE WHERE SAMPLE_ID = 999") {
        Ok(n) => eprintln!("  execute(DELETE): affected={}", n),
        Err(e) => eprintln!("  execute(DELETE): ERROR: {}", e),
    }
}
