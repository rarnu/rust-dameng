use dameng::Client;
fn main() {
    let mut s = Client::new("127.0.0.1", 5236);
    s.connect("SYSDBA", "SYSDBA").unwrap();
    s.execute("DROP TABLE IF EXISTS SAMPLEX").unwrap();
    s.execute("CREATE TABLE SAMPLEX (id INT PRIMARY KEY, name VARCHAR(50) NOT NULL, age INTEGER DEFAULT 0, address VARCHAR(128), create_time TIMESTAMP, height DECIMAL, birthday DATE)").unwrap();

    let ts = chrono::NaiveDateTime::parse_from_str("2024-06-15 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    s.execute_with_params("INSERT INTO SAMPLEX VALUES(?,?,?,?,?,?,?)",
        &[&1, &"Alice", &18, &"addr1", &ts, &rust_decimal::Decimal::from_str_exact("175.5").unwrap(), &chrono::NaiveDate::from_ymd_opt(1990,1,15).unwrap()]).unwrap();
    s.execute_with_params("INSERT INTO SAMPLEX VALUES(?,?,?,?,?,?,?)",
        &[&2, &"Bob", &20, &"addr2", &ts, &rust_decimal::Decimal::from_str_exact("180").unwrap(), &Option::<chrono::NaiveDate>::None]).unwrap();

    let rs = s.query("SELECT * FROM SAMPLEX").unwrap();
    println!("cols={}", rs.columns.len());
    for row in rs.iter() {
        let id: i32 = row.get(0).unwrap();
        let name: &str = row.get(1).unwrap();
        let address: Option<&str> = row.get(3).unwrap();
        let cd: Option<chrono::NaiveDateTime> = row.get(4).unwrap();
        let height: Option<rust_decimal::Decimal> = row.get(5).unwrap();
        let dt: Option<chrono::NaiveDate> = row.get(6).unwrap();
        println!("person {}, {}, {:?}, {:?}, {:?}, {:?}", id, name, address, cd, height, dt);
    }
    s.execute("DROP TABLE SAMPLEX").ok();
    s.close().unwrap();
}
