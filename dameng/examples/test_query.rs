//! Test query with SQLx-style API.
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("DM_PORT").unwrap_or_else(|_| "5236".to_string()).parse().unwrap_or(5236);
    let user = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".to_string());
    let pass = env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".to_string());

    let mut client = dameng::Client::new(&host, port);
    client.connect(&user, &pass)?;

    let rs = client.query("SELECT ID, NAME, ADDRESS FROM SAMPLE")?;
    for row in rs.iter_rows() {
        let id: i32 = row.get(0)?;
        let name: &str = row.get(1)?;
        let address: Option<&str> = row.get(2)?;
        println!("person: {} {} {:?}", id, name, address);
    }

    client.close()?;
    Ok(())
}
