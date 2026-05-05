use std::env;

fn main() {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = env::var("DM_PORT")
        .unwrap_or_else(|_| "5236".to_string())
        .parse()
        .unwrap_or(5236);
    let username = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".to_string());
    let password = env::var("DM_PASS").unwrap_or_else(|_| "".to_string());

    println!("Testing Dameng sync driver against {}:{} ...", host, port);
    println!("User: {}", username);

    // Test 1: Sync client connection
    println!("\n=== Test 1: Sync connect ===");
    let mut client = dameng::Client::new(&host, port);
    match client.connect(&username, &password) {
        Ok(()) => println!("OK: Connected successfully"),
        Err(e) => {
            println!("FAIL: {}", e);
            return;
        }
    }

    // Test 2: Execute SELECT 1
    println!("\n=== Test 2: SELECT 1 ===");
    match client.execute("SELECT 1") {
        Ok(rows) => {
            println!("OK: Got {} row(s)", rows.len());
            if let Some(row) = rows.first() {
                println!("  First row has {} columns", row.len());
                if let Some(val) = row.get_i32(0) {
                    println!("  Column 0 = {}", val);
                }
            }
        }
        Err(e) => {
            println!("FAIL: {}", e);
            return;
        }
    }

    // Test 3: Query system version
    println!("\n=== Test 3: SELECT VERSION FROM V$VERSION ===");
    match client.execute("SELECT VERSION FROM V$VERSION") {
        Ok(rows) => {
            println!("OK: Got {} row(s)", rows.len());
            if let Some(row) = rows.first() {
                if let Some(ver) = row.get_str(0) {
                    println!("  DM Version: {}", ver);
                }
            }
        }
        Err(e) => {
            println!("WARN: {}", e);
        }
    }

    // Test 4: Ready/keepalive
    println!("\n=== Test 4: READY keepalive ===");
    match client.ready() {
        Ok(()) => println!("OK: READY acknowledged"),
        Err(e) => {
            println!("FAIL: {}", e);
            return;
        }
    }

    // Test 5: Commit
    println!("\n=== Test 5: COMMIT ===");
    match client.commit() {
        Ok(()) => println!("OK: COMMIT succeeded"),
        Err(e) => {
            println!("WARN: {}", e);
        }
    }

    // Test 6: Async client
    println!("\n=== Test 6: Async connect ===");
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut async_client = tokio_dameng::Client::new(&host, port);
        match async_client.connect(&username, &password).await {
            Ok(()) => {
                println!("OK: Async client connected");

                // Execute SELECT 1 via async
                match async_client.execute("SELECT 1").await {
                    Ok(rows) => {
                        println!("OK: Async got {} row(s)", rows.len());
                        if let Some(row) = rows.first() {
                            if let Some(val) = row.get_i32(0) {
                                println!("  Column 0 = {}", val);
                            }
                        }
                    }
                    Err(e) => {
                        println!("FAIL: Async execute: {}", e);
                    }
                }

                match async_client.commit().await {
                    Ok(()) => println!("OK: Async commit succeeded"),
                    Err(e) => println!("WARN: Async commit: {}", e),
                }
            }
            Err(e) => {
                println!("FAIL: Async connect: {}", e);
            }
        }
    });

    println!("\n=== All tests completed ===");
}
