//! Test query_with_params with &[&i32, &&str] — SQLx-style.
//!
//! Usage: cargo run --example test_qwp

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = dameng::Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA")?;

    // 用户期望的调用方式
    let id: i32 = 1;
    let name: &str = "Alice";
    let rs = client.query_with_params(
        "SELECT ID, NAME, ADDRESS FROM SAMPLE WHERE ID = ? AND NAME = ?",
        &[&id, &name],
    )?;
    println!("total_row_count={}", rs.total_row_count);
    for row in rs.iter() {
        let id_val: i32 = row.get(0).unwrap_or_default();
        let name_val = row.get_str(1).unwrap_or("<NULL>");
        let addr = row.get_str(2).unwrap_or("<NULL>");
        println!("  ID={}, NAME={}, ADDR={}", id_val, name_val, addr);
    }

    println!("\nAll query_with_params tests passed!");
    Ok(())
}
