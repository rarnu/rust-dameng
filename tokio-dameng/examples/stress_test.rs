//! Concurrent pool stress test.
//!
//! Spawns N tasks that each checkout a connection from the pool,
//! execute queries, and drop the connection. Verifies that all
//! connections are properly returned.

use std::time::Instant;
use tokio::task::JoinSet;
use tokio_dameng::{Pool, PoolConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let host = std::env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("DM_PORT")
        .unwrap_or_else(|_| "5236".into())
        .parse()
        .unwrap_or(5236);
    let user = std::env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".into());
    let pass = std::env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".into());

    println!("=== Pool Stress Test ===");
    println!("Host: {}:{} User: {}", host, port, user);

    let config = PoolConfig {
        min_idle: 2,
        max_size: 10,
        max_lifetime: Some(std::time::Duration::from_secs(300)),
        wait_timeout: std::time::Duration::from_secs(30),
        idle_check_interval: std::time::Duration::from_secs(30),
    };

    let pool = Pool::new(&host, port, &user, &pass, config);
    println!("Pool created (max_size={})", pool.max_size());

    // Test 1: Sequential queries
    println!("\n--- Test 1: Sequential queries (20x) ---");
    let start = Instant::now();
    for i in 0..20 {
        let mut conn = pool.get().await?;
        let rs = conn.query("SELECT 1").await?;
        let val = rs.rows.first().ok_or("empty")?.get_i32(0)?;
        assert_eq!(val, 1, "expected 1 got {}", val);
        drop(conn);
        if i % 5 == 0 {
            println!("  seq query {}/20 OK", i + 1);
        }
    }
    let elapsed = start.elapsed();
    println!(
        "Sequential: 20 queries in {:?} (~{:.2}ms/query)",
        elapsed,
        elapsed.as_micros() as f64 / 20.0 / 1000.0
    );

    // Test 2: Concurrent queries (up to pool max)
    println!("\n--- Test 2: Concurrent queries (10x, pool max=10) ---");
    let start = Instant::now();
    let mut set = JoinSet::new();
    for i in 0..10 {
        let pool = pool.clone();
        set.spawn(async move {
            let mut conn = pool.get().await.unwrap();
            let rs = conn.query(&format!("SELECT {} AS NUM", i)).await.unwrap();
            let val = rs.rows.first().unwrap().get_i32(0).unwrap();
            assert_eq!(val, i);
            drop(conn);
            i
        });
    }
    let mut count = 0;
    while let Some(res) = set.join_next().await {
        res?;
        count += 1;
    }
    let elapsed = start.elapsed();
    println!(
        "Concurrent: 10 queries in {:?} (~{:.2}ms/query)",
        elapsed,
        elapsed.as_micros() as f64 / 10.0 / 1000.0
    );
    assert_eq!(count, 10);

    // Test 3: Burst concurrency (pool saturates)
    println!(
        "\n--- Test 3: Burst (20 concurrent, pool max=10, sem limits to 10) ---"
    );
    let start = Instant::now();
    let mut set = JoinSet::new();
    for i in 0..20 {
        let pool = pool.clone();
        set.spawn(async move {
            let mut conn = pool.get().await.unwrap();
            let rs = conn.query("SELECT 42").await.unwrap();
            let val = rs.rows.first().unwrap().get_i32(0).unwrap();
            assert_eq!(val, 42);
            drop(conn);
            i
        });
    }
    let mut count = 0;
    while let Some(res) = set.join_next().await {
        res?;
        count += 1;
    }
    let elapsed = start.elapsed();
    println!(
        "Burst: 20 queries in {:?} (~{:.2}ms/query)",
        elapsed,
        elapsed.as_micros() as f64 / 20.0 / 1000.0
    );
    assert_eq!(count, 20);

    // Verify pool state after tests
    println!("\n--- Pool State After Tests ---");
    println!("Available permits: {}", pool.available_permits());
    println!("Max size: {}", pool.max_size());

    // Test 4: Clone pool and use from different tasks
    println!("\n--- Test 4: Clone pool (3 clones, 5 queries each) ---");
    let start = Instant::now();
    let mut set = JoinSet::new();
    for c in 0..3 {
        let pool = pool.clone();
        set.spawn(async move {
            for i in 0..5 {
                let mut conn = pool.get().await.unwrap();
                let rs =
                    conn.query(&format!("SELECT {} + {}", c, i)).await.unwrap();
                let val = rs.rows.first().unwrap().get_i32(0).unwrap();
                assert_eq!(val, c + i);
                drop(conn);
            }
            c
        });
    }
    let mut count = 0;
    while let Some(res) = set.join_next().await {
        res?;
        count += 1;
    }
    let elapsed = start.elapsed();
    println!(
        "Clone: 15 queries in {:?} (~{:.2}ms/query)",
        elapsed,
        elapsed.as_micros() as f64 / 15.0 / 1000.0
    );
    assert_eq!(count, 3);

    println!("\n=== All Pool Stress Tests PASSED ===");
    Ok(())
}
