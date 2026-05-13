const HOST: &str = "127.0.0.1";
const PORT: u16 = 5236;
const USER: &str = "SYSDBA";
const PASS: &str = "SYSDBA";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = dameng::Client::new(HOST, PORT);
    client.connect(USER, PASS)?;
    println!("Connected!\n");

    // =========================================================
    // 1. 准备测试表
    // =========================================================
    println!("=== 1. Setup test table ===");
    let _ = client.execute("DROP TABLE IF EXISTS dm_test_ar");
    client.execute(
        "CREATE TABLE dm_test_ar (
            ID INT PRIMARY KEY,
            NAME VARCHAR(50),
            CNT INT
        )",
    )?;
    println!("  Created dm_test_ar\n");

    // =========================================================
    // 2. INSERT — 单条
    // =========================================================
    println!("=== 2. INSERT single row ===");
    let r = client.execute(
        "INSERT INTO dm_test_ar (ID, NAME, CNT) VALUES (1, 'Alice', 10)",
    )?;
    println!("  Expected: 1, Got: {}", r);
    assert_eq!(r, 1, "INSERT single should return 1");

    // =========================================================
    // 3. INSERT — 多条
    // =========================================================
    println!("\n=== 3. INSERT multiple rows ===");
    let r = client.execute(
        "INSERT INTO dm_test_ar (ID, NAME, CNT) VALUES \
         (2, 'Bob', 20), (3, 'Charlie', 30), (4, 'David', 40)",
    )?;
    println!("  Expected: 3, Got: {}", r);
    assert_eq!(r, 3, "INSERT multiple should return 3");

    // 验证总行数
    let rs = client.query("SELECT COUNT(*) FROM dm_test_ar")?;
    let total = rs.iter().next().unwrap().get_i32(0).unwrap();
    println!("  Total rows in table: {}", total);
    assert_eq!(total, 4, "Should have 4 rows");

    // =========================================================
    // 4. UPDATE — 单条
    // =========================================================
    println!("\n=== 4. UPDATE single row ===");
    let r = client.execute(
        "UPDATE dm_test_ar SET NAME = 'Alice2', CNT = 100 WHERE ID = 1",
    )?;
    println!("  Expected: 1, Got: {}", r);
    assert_eq!(r, 1, "UPDATE single should return 1");

    // 验证更新结果
    let rs = client.query("SELECT NAME, CNT FROM dm_test_ar WHERE ID = 1")?;
    let row = rs.iter().next().unwrap();
    let name = row.get_str(0).unwrap();
    let cnt = row.get_i32(1).unwrap();
    println!("  Verified: NAME='{}', CNT={}", name, cnt);
    assert_eq!(name, "Alice2");
    assert_eq!(cnt, 100);

    // =========================================================
    // 5. UPDATE — 多条
    // =========================================================
    println!("\n=== 5. UPDATE multiple rows ===");
    let r = client.execute(
        "UPDATE dm_test_ar SET CNT = CNT + 1000 WHERE ID >= 2 AND ID <= 4",
    )?;
    println!("  Expected: 3, Got: {}", r);
    assert_eq!(r, 3, "UPDATE multiple should return 3");

    // =========================================================
    // 6. UPDATE — 零条 (WHERE 不匹配)
    // =========================================================
    println!("\n=== 6. UPDATE zero rows (no match) ===");
    let r = client.execute(
        "UPDATE dm_test_ar SET NAME = 'NoMatch' WHERE ID = 999",
    )?;
    println!("  Expected: 0, Got: {}", r);
    assert_eq!(r, 0, "UPDATE no match should return 0");

    // =========================================================
    // 7. DELETE — 单条
    // =========================================================
    println!("\n=== 7. DELETE single row ===");
    let r = client.execute("DELETE FROM dm_test_ar WHERE ID = 2")?;
    println!("  Expected: 1, Got: {}", r);
    assert_eq!(r, 1, "DELETE single should return 1");

    // =========================================================
    // 8. DELETE — 多条
    // =========================================================
    println!("\n=== 8. DELETE multiple rows ===");
    let r =
        client.execute("DELETE FROM dm_test_ar WHERE ID IN (3, 4)")?;
    println!("  Expected: 2, Got: {}", r);
    assert_eq!(r, 2, "DELETE multiple should return 2");

    // =========================================================
    // 9. DELETE — 零条
    // =========================================================
    println!("\n=== 9. DELETE zero rows (no match) ===");
    let r = client.execute("DELETE FROM dm_test_ar WHERE ID = 999")?;
    println!("  Expected: 0, Got: {}", r);
    assert_eq!(r, 0, "DELETE no match should return 0");

    // 验证剩余行数
    let rs = client.query("SELECT COUNT(*) FROM dm_test_ar")?;
    let total = rs.iter().next().unwrap().get_i32(0).unwrap();
    println!("\n  Remaining rows: {}", total);
    assert_eq!(total, 1, "Should have 1 row left");

    // =========================================================
    // 10. 清理
    // =========================================================
    println!("\n=== 10. Cleanup ===");
    let _ = client.execute("DROP TABLE dm_test_ar");
    println!("  Dropped dm_test_ar");

    println!("\nAll tests passed!");
    client.close()?;
    Ok(())
}
