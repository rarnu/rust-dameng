use dameng::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Client::new("127.0.0.1", 5236);
    client.connect("SYSDBA", "SYSDBA")?;
    println!("Connected!\n");

    let queries = vec![
        ("SELECT CURRENT_DATE FROM DUAL", "CURRENT_DATE"),
        ("SELECT CURRENT_TIME FROM DUAL", "CURRENT_TIME"),
        ("SELECT CURRENT_TIMESTAMP FROM DUAL", "CURRENT_TIMESTAMP"),
        ("SELECT SYSDATE FROM DUAL", "SYSDATE"),
        (
            "SELECT TO_DATE('2024-01-01', 'YYYY-MM-DD') FROM DUAL",
            "TO_DATE",
        ),
        (
            "SELECT TO_TIMESTAMP('2024-01-01 12:00:00', 'YYYY-MM-DD HH24:MI:SS') FROM DUAL",
            "TO_TIMESTAMP",
        ),
    ];

    for (sql, label) in queries {
        match client.query(sql) {
            Ok(rs) => {
                for col in &rs.columns {
                    println!(
                        "{}: col_name={} type_name={} type_code={} precision={} scale={}",
                        label, col.name, col.type_name, col.type_code, col.precision, col.scale
                    );
                }
                for row in &rs.rows {
                    if let Some(Some(val)) = row.values.get(0) {
                        let hex: String = val.iter().map(|b| format!("{:02X}", b)).collect();
                        println!(
                            "  value ({} bytes): {} -> {:?}",
                            val.len(),
                            &hex[..hex.len().min(80)],
                            String::from_utf8_lossy(val)
                        );
                    }
                }
            }
            Err(e) => {
                println!("{}: ERROR: {}", label, e);
            }
        }
    }

    println!();

    // Check INTERVAL types
    match client.query("SELECT NUMTODSINTERVAL(1, 'DAY') FROM DUAL") {
        Ok(rs) => {
            for col in &rs.columns {
                println!(
                    "INTERVAL_DT: col_name={} type_code={} precision={} scale={}",
                    col.name, col.type_code, col.precision, col.scale
                );
            }
            for row in &rs.rows {
                if let Some(Some(val)) = row.values.get(0) {
                    println!(
                        "  value ({} bytes): {:?}",
                        val.len(),
                        String::from_utf8_lossy(val)
                    );
                }
            }
        }
        Err(e) => println!("INTERVAL_DT: ERROR: {}", e),
    }

    match client.query("SELECT NUMTOYMINTERVAL(1, 'YEAR') FROM DUAL") {
        Ok(rs) => {
            for col in &rs.columns {
                println!(
                    "INTERVAL_YM: col_name={} type_code={} precision={} scale={}",
                    col.name, col.type_code, col.precision, col.scale
                );
            }
            for row in &rs.rows {
                if let Some(Some(val)) = row.values.get(0) {
                    println!(
                        "  value ({} bytes): {:?}",
                        val.len(),
                        String::from_utf8_lossy(val)
                    );
                }
            }
        }
        Err(e) => println!("INTERVAL_YM: ERROR: {}", e),
    }

    Ok(())
}
