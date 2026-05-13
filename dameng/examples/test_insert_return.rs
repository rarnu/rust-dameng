const HOST: &str = "127.0.0.1";
const PORT: u16 = 5236;
const USER: &str = "SYSDBA";
const PASS: &str = "SYSDBA";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = dameng::Client::new(HOST, PORT);
    client.connect(USER, PASS)?;
    println!("Connected!\n");

    // First cleanup
    let _ = client.execute("DELETE FROM sample WHERE ID = 5");

    let r1 = client.execute("INSERT INTO sample (ID, NAME) VALUES (5, 'Emma')")?;
    println!("Inserted {} rows into sample.", r1);

    client.close()?;
    Ok(())
}
