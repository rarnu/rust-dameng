//! Async example showing the new DmDecode + IntoIterator API.
//!
//! Usage: DM_HOST=127.0.0.1 DM_PORT=5236 DM_USER=SYSDBA DM_PASS=SYSDBA cargo run --example async_query -p tokio-dameng

use tokio_dameng::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("DM_PORT")
        .unwrap_or_else(|_| "5236".into())
        .parse()?;
    let user = std::env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".into());
    let pass = std::env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".into());

    let mut client = Client::new(&host, port);
    client.connect(&user, &pass).await?;
    println!("Connected to Dameng as {}", user);

    // Q1: Consuming iterator — IntoIterator
    println!("\nQ1: for row in rs (into_iter)");
    let rs = client.query("SELECT ID, NAME, ADDRESS FROM SAMPLE WHERE ID <= 3 ORDER BY ID").await?;
    for row in rs {
        let id: i32 = row.get(0)?;
        let name: &str = row.get(1)?;
        let addr: Option<&str> = row.get(2)?;
        println!("  {} {} {:?}", id, name, addr);
    }
    // rs is consumed

    // Q2: Borrowing iterator — iter_rows()
    println!("\nQ2: for row in rs.iter_rows()");
    let rs = client.query("SELECT ID, NAME FROM SAMPLE WHERE ID <= 3 ORDER BY ID").await?;
    for row in rs.iter_rows() {
        let id: i32 = row.get(0)?;
        let name: &str = row.get(1)?;
        println!("  {} {}", id, name);
    }

    // Q3: Option<&str> for nullable columns
    println!("\nQ3: Option<&str>");
    let rs = client.query("SELECT ID, ADDRESS FROM SAMPLE WHERE ID <= 3 ORDER BY ID").await?;
    for row in rs.iter_rows() {
        let addr: Option<&str> = row.get(1)?;
        println!("  {} {:?}", row.get::<i32>(0)?, addr);
    }

    // Q4: Scalar queries (count, etc.)
    println!("\nQ4: Scalar");
    let rs = client.query("SELECT COUNT(*) AS CNT FROM SAMPLE").await?;
    for row in rs {
        let count: i32 = row.get(0)?;
        println!("  count={}", count);
    }

    // Q5: DML with affected rows
    println!("\nQ5: DML execute()");
    let affected = client.execute("UPDATE SAMPLE SET NAME = NAME WHERE ID = 1").await?;
    println!("  affected={}", affected);

    // Q6: Direct Row access via ResultSet methods
    println!("\nQ6: ResultSet.first() and old-style access");
    let rs = client.query("SELECT ID, NAME FROM SAMPLE WHERE ID = 1").await?;
    if let Some(row) = rs.first() {
        let id = row.get_i32(0)?;
        let name = row.get_str(1)?;
        println!("  first: {} {}", id, name);
    }

    println!("\nALL OK!");
    Ok(())
}
