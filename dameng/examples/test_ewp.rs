//! Test execute_with_params for INSERT, UPDATE, DELETE — SQLx-style.
//!
//! Usage: cargo run --example test_ewp

const HOST: &str = "127.0.0.1";
const PORT: u16 = 5236;
const USER: &str = "SYSDBA";
const PASS: &str = "SYSDBA";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = dameng::Client::new(HOST, PORT);
    client.connect(USER, PASS)?;
    println!("Connected!\n");

    // === INSERT ===
    let id: i32 = 99;
    let name: &str = "TestUser";
    let age: i32 = 25;
    let addr: &str = "TestAddr";
    let r1 = client.execute_with_params(
        "INSERT INTO SAMPLE (ID, NAME, AGE, ADDRESS) VALUES (?, ?, ?, ?)",
        &[&id, &name, &age, &addr],
    )?;
    println!("INSERT result: {} row(s) affected", r1);
    assert!(r1 >= 1, "INSERT should affect >= 1 row");

    // Verify INSERT
    let rs = client.query_with_params(
        "SELECT ID, NAME, AGE, ADDRESS FROM SAMPLE WHERE ID = ?",
        &[&id],
    )?;
    assert!(rs.total_row_count >= 1, "INSERTed row should exist");
    for row in rs.iter() {
        let rid: i32 = row.get(0).unwrap_or_default();
        let rname = row.get_str(1).unwrap_or("<NULL>");
        let rage: i32 = row.get(2).unwrap_or_default();
        let raddr = row.get_str(3).unwrap_or("<NULL>");
        println!("  Found: ID={}, NAME={}, AGE={}, ADDR={}", rid, rname, rage, raddr);
        assert_eq!(rid, 99);
        assert_eq!(rname, "TestUser");
        assert_eq!(rage, 25);
        assert_eq!(raddr, "TestAddr");
    }
    println!("INSERT verified!\n");

    // === UPDATE ===
    let new_name: &str = "UpdatedUser";
    let new_age: i32 = 30;
    let r2 = client.execute_with_params(
        "UPDATE SAMPLE SET NAME = ?, AGE = ? WHERE ID = ?",
        &[&new_name, &new_age, &id],
    )?;
    println!("UPDATE result: {} row(s) affected", r2);
    assert!(r2 >= 1, "UPDATE should affect >= 1 row");

    // Verify UPDATE
    let rs = client.query_with_params(
        "SELECT NAME, AGE FROM SAMPLE WHERE ID = ?",
        &[&id],
    )?;
    for row in rs.iter() {
        let uname = row.get_str(0).unwrap_or("<NULL>");
        let uage: i32 = row.get(1).unwrap_or_default();
        println!("  Updated: NAME={}, AGE={}", uname, uage);
        assert_eq!(uname, "UpdatedUser");
        assert_eq!(uage, 30);
    }
    println!("UPDATE verified!\n");

    // === DELETE ===
    let r3 = client.execute_with_params(
        "DELETE FROM SAMPLE WHERE ID = ?",
        &[&id],
    )?;
    println!("DELETE result: {} row(s) affected", r3);
    assert!(r3 >= 1, "DELETE should affect >= 1 row");

    // Verify DELETE
    let rs = client.query_with_params(
        "SELECT COUNT(*) as cnt FROM SAMPLE WHERE ID = ?",
        &[&id],
    )?;
    for row in rs.iter() {
        let cnt: i32 = row.get(0).unwrap_or_default();
        println!("  After DELETE, matching rows: {}", cnt);
        assert_eq!(cnt, 0, "Row should be deleted");
    }
    println!("DELETE verified!\n");

    client.close()?;
    println!("\nAll execute_with_params tests passed! ฅ^•ﻌ•^ฅ");
    Ok(())
}
