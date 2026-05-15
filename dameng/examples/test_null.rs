use dameng::Client;
fn main() {
    let mut c = Client::new("127.0.0.1", 5236);
    c.connect("SYSDBA", "SYSDBA").unwrap();
    c.execute("DROP TABLE IF EXISTS T_NULL").unwrap();
    c.execute("CREATE TABLE T_NULL (id INT PRIMARY KEY, bday DATE)").unwrap();
    c.execute("INSERT INTO T_NULL VALUES(1, '2000-06-15')").unwrap();
    c.execute("INSERT INTO T_NULL VALUES(2, NULL)").unwrap();
    let rs = c.query("SELECT bday FROM T_NULL").unwrap();
    for row in rs.iter_rows() {
        if let Some(ref b) = row.row.values[0] {
            println!("bday bytes({}): {:02x?}", b.len(), b);
            let bd: Option<chrono::NaiveDate> = row.get(0).unwrap();
            println!("  -> {:?}", bd);
        } else { println!("bday: NULL"); }
    }
    c.execute("DROP TABLE T_NULL").ok();
    c.close().unwrap();
}
