// Quick debug: test DML affected rows
use dameng::{Client, ResultSet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = "127.0.0.1";
    let port = 5236;
    let user = "SYSDBA";
    let pass = "SYSDBA";

    let mut client = Client::new(host, port);
    client.connect(user, pass)?;
    println!("Connected!");

    // INSERT test
    let r1 = client.execute("INSERT INTO SAMPLE (ID, NAME) VALUES (99, 'DebugTest')")?;
    println!("INSERT affected: {}", r1);

    // UPDATE test
    let r2 = client.execute("UPDATE SAMPLE SET NAME='Updated' WHERE ID=1")?;
    println!("UPDATE affected: {}", r2);

    // DELETE test
    let r3 = client.execute("DELETE FROM SAMPLE WHERE ID=99")?;
    println!("DELETE affected: {}", r3);

    client.close()?;
    Ok(())
}