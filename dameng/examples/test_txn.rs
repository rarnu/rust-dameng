use dameng::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ── User's exact pattern: commit(self), client usable after ──
    {
        let mut client = Client::new("127.0.0.1", 5236);
        client.connect("SYSDBA", "SYSDBA")?;
        println!("Test 1: commit(self), client after");

        let mut tx = client.transaction()?;
        tx.execute_with_params(
            "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
            &[&301i32, &"UserA", &20i32, &"AddrA"],
        )?;
        tx.commit()?; // tx consumed, borrow released

        // client works immediately!
        let rs = client.query_with_params("SELECT NAME FROM SAMPLE WHERE ID = ?", &[&301i32])?;
        for row in rs.iter() {
            println!("  commit found: NAME={}", row.get_str(0).unwrap_or("<NULL"));
        }
        // Clean up
        client.execute_with_params("DELETE FROM SAMPLE WHERE ID = ?", &[&301i32])?;
        client.close()?;
    }
    println!("Test 1 PASSED ✓\n");

    // ── rollback(self) ──
    {
        let mut client = Client::new("127.0.0.1", 5236);
        client.connect("SYSDBA", "SYSDBA")?;
        println!("Test 2: rollback(self)");

        let mut tx = client.transaction()?;
        tx.execute_with_params(
            "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
            &[&401i32, &"UserB", &30i32, &"AddrB"],
        )?;
        tx.rollback()?; // tx consumed, borrow released

        let rs = client.query_with_params("SELECT COUNT(*) AS c FROM SAMPLE WHERE ID = ?", &[&401i32])?;
        for row in rs.iter() {
            assert_eq!(row.get::<i32>(0).unwrap_or_default(), 0);
            println!("  after rollback: count=0 ✓");
        }
        client.close()?;
    }
    println!("Test 2 PASSED ✓\n");

    // ── Drop auto-rollback ──
    {
        let mut client = Client::new("127.0.0.1", 5236);
        client.connect("SYSDBA", "SYSDBA")?;
        println!("Test 3: Drop auto-rollback");

        {
            let mut tx = client.transaction()?;
            tx.execute_with_params(
                "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
                &[&501i32, &"UserC", &40i32, &"AddrC"],
            )?;
            // No commit → Drop auto-rollbacks
        }
        // client available after Drop (borrow released by scope)
        let rs = client.query_with_params("SELECT COUNT(*) AS c FROM SAMPLE WHERE ID = ?", &[&501i32])?;
        for row in rs.iter() {
            assert_eq!(row.get::<i32>(0).unwrap_or_default(), 0);
            println!("  after drop: count=0 ✓");
        }
        client.close()?;
    }
    println!("Test 3 PASSED ✓\n");

    // ── Multiple transactions on same client ──
    {
        let mut client = Client::new("127.0.0.1", 5236);
        client.connect("SYSDBA", "SYSDBA")?;
        println!("Test 4: multiple transactions");

        let mut tx1 = client.transaction()?;
        tx1.execute_with_params("INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)", &[&601i32, &"Tx1", &10i32, &"A1"])?;
        tx1.commit()?;

        let mut tx2 = client.transaction()?;
        tx2.execute_with_params("INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)", &[&602i32, &"Tx2", &20i32, &"A2"])?;
        tx2.rollback()?;

        // tx1 data exists, tx2 data does not
        let rs1 = client.query_with_params("SELECT NAME FROM SAMPLE WHERE ID = ?", &[&601i32])?;
        let mut found = false;
        for row in rs1.iter() { println!("  tx1: NAME={}", row.get_str(0).unwrap_or("")); found = true; }
        assert!(found);

        let rs2 = client.query_with_params("SELECT COUNT(*) FROM SAMPLE WHERE ID = ?", &[&602i32])?;
        for row in rs2.iter() { assert_eq!(row.get::<i32>(0).unwrap_or(-1), 0); }

        // Clean up
        client.execute_with_params("DELETE FROM SAMPLE WHERE ID = ?", &[&601i32])?;
        client.close()?;
    }
    println!("Test 4 PASSED ✓\n");

    println!("All transaction tests passed! (=^ΦωΦ^=)");
    Ok(())
}
