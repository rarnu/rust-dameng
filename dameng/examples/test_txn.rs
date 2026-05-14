use dameng::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── Test 1: commit ──
    {
        let mut client = Client::new("127.0.0.1", 5236);
        client.connect("SYSDBA", "SYSDBA")?;
        println!("Test 1: COMMIT");
        let mut tx = client.transaction()?;
        tx.execute_with_params(
            "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
            &[&300i32, &"CommitTest", &99i32, &"CommitAddr"],
        )?;
        let mut c = tx.commit()?;
        println!("  committed!");
        let rs = c.query_with_params("SELECT NAME FROM SAMPLE WHERE ID = ?", &[&300i32])?;
        let mut found = false;
        for row in rs.iter() {
            println!("  found: NAME={}", row.get_str(0).unwrap_or("<NULL>"));
            found = true;
        }
        assert!(found);
        let mut tx = c.transaction()?;
        tx.execute_with_params("DELETE FROM SAMPLE WHERE ID = ?", &[&300i32])?;
        let _ = tx.commit()?;
    }
    println!("Test 1 PASSED\n");

    // ── Test 2: rollback ──
    {
        let mut client = Client::new("127.0.0.1", 5236);
        client.connect("SYSDBA", "SYSDBA")?;
        println!("Test 2: ROLLBACK");
        let mut tx = client.transaction()?;
        tx.execute_with_params(
            "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
            &[&400i32, &"RollbackTest", &50i32, &"RollbackAddr"],
        )?;
        let mut c = tx.rollback()?;
        println!("  rolled back!");
        let rs = c.query_with_params("SELECT COUNT(*) AS c FROM SAMPLE WHERE ID = ?", &[&400i32])?;
        for row in rs.iter() {
            let cnt: i32 = row.get(0).unwrap_or_default();
            assert_eq!(cnt, 0);
            println!("  count after rollback: {}", cnt);
        }
        c.close()?;
    }
    println!("Test 2 PASSED\n");

    // ── Test 3: Drop auto-rollback ──
    {
        println!("Test 3: Drop auto-rollback");
        {
            let mut client = Client::new("127.0.0.1", 5236);
            client.connect("SYSDBA", "SYSDBA")?;
            let mut tx = client.transaction()?;
            tx.execute_with_params(
                "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
                &[&500i32, &"DropTest", &60i32, &"DropAddr"],
            )?;
            println!("  inserted, dropping tx (auto-rollback)...");
            // tx dropped here → auto-rollback, client consumed
        }
        // Use a fresh client to verify the row was NOT committed
        let mut c2 = Client::new("127.0.0.1", 5236);
        c2.connect("SYSDBA", "SYSDBA")?;
        let rs = c2.query_with_params("SELECT COUNT(*) AS c FROM SAMPLE WHERE ID = ?", &[&500i32])?;
        for row in rs.iter() {
            let cnt: i32 = row.get(0).unwrap_or_default();
            assert_eq!(cnt, 0);
            println!("  count after drop: {}", cnt);
        }
        c2.close()?;
    }
    println!("Test 3 PASSED\n");

    // ── Test 4: batch rollback ──
    {
        let mut client = Client::new("127.0.0.1", 5236);
        client.connect("SYSDBA", "SYSDBA")?;
        println!("Test 4: batch rollback");
        let mut tx = client.transaction()?;
        for id in 600..605 {
            let name = format!("Batch{}", id);
            tx.execute_with_params(
                "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
                &[&id, &name.as_str(), &25i32, &"BatchAddr"],
            )?;
        }
        println!("  inserted 5 rows, rolling back...");
        let mut c = tx.rollback()?;
        for id in 600..605 {
            let rs = c.query_with_params("SELECT COUNT(*) AS c FROM SAMPLE WHERE ID = ?", &[&id])?;
            for row in rs.iter() {
                let cnt: i32 = row.get(0).unwrap_or_default();
                assert_eq!(cnt, 0);
            }
        }
        println!("  all 5 rows rolled back!");
        c.close()?;
    }
    println!("Test 4 PASSED\n");

    println!("All transaction tests passed! (=^･ω･^=)");
    Ok(())
}
