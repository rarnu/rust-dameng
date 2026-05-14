fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = dameng::Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA")?;

    let id: i32 = 2;
    let rs = client.query_with_params("SELECT ID, NAME, ADDRESS FROM SAMPLE WHERE ID > ?", &[&id])?;
    println!("total: {}, columns: {}", rs.total_row_count, rs.columns.len());
    for row in rs.iter() {
        let rid: i32 = row.get(0)?;
        let name: &str = row.get(1)?;
        println!("person {}, {}", rid, name);
    }
    Ok(())
}
