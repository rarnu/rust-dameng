//! Comprehensive integration tests against live DM instance.

use std::env;

type TR = Result<String, String>;

fn run(name: &str, r: TR, p: &mut i32, f: &mut i32) {
    match r {
        Ok(m) => { println!("PASS: {} ({})", name, m); *p += 1; }
        Err(e) => { println!("FAIL: {} - {}", name, e); *f += 1; }
    }
}

fn exec(c: &mut dameng::Client, sql: &str) -> TR {
    c.execute(sql).map(|_| "ok".into()).map_err(|e| e.to_string())
}

async fn aexec(c: &mut tokio_dameng::Client, sql: &str) -> TR {
    c.execute(sql).await.map(|_| "ok".into()).map_err(|e| e.to_string())
}

fn g32(rs: &dameng::row::ResultSet) -> TR {
    rs.first().ok_or("no row".to_string())?.get_i32(0).map_err(|e| e.to_string()).map(|v| v.to_string())
}

fn gs(rs: &dameng::row::ResultSet) -> TR {
    rs.first().ok_or("no row".to_string())?.get_str(0).map_err(|e| e.to_string())
}

fn main() {
    let host = env::var("DM_HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = env::var("DM_PORT").unwrap_or_else(|_| "5236".into()).parse().unwrap_or(5236);
    let user = env::var("DM_USER").unwrap_or_else(|_| "SYSDBA".into());
    let pass = env::var("DM_PASS").unwrap_or_else(|_| "SYSDBA".into());
    let mut p: i32 = 0;
    let mut f: i32 = 0;

    println!("\n=== Sync Client Tests ===");
    let mut c = dameng::Client::new(&host, port);
    run("connect", c.connect(&user, &pass).map(|_| "ok".into()).map_err(|e| e.to_string()), &mut p, &mut f);
    run("SELECT 1", (|| {
        let rs = c.execute("SELECT 1").map_err(|e| e.to_string())?;
        assert_eq!(rs.len(), 1);
        let v = g32(&rs)?; assert_eq!(v, "1");
        Ok("ok".into())
    })(), &mut p, &mut f);
    run("V$VERSION", (|| {
        let rs = c.execute("SELECT * FROM V$VERSION").map_err(|e| e.to_string())?;
        let ver = gs(&rs)?; assert!(!ver.is_empty());
        Ok(ver)
    })(), &mut p, &mut f);

    println!("\n=== CRUD Tests ===");
    run("CREATE TABLE", (|| {
        let _ = exec(&mut c, "DROP TABLE IF EXISTS RUST_TEST");
        exec(&mut c, "CREATE TABLE RUST_TEST (ID INT PRIMARY KEY, NAME VARCHAR(100), AGE INT)")
    })(), &mut p, &mut f);
    run("INSERT", (|| {
        let _ = exec(&mut c, "INSERT INTO RUST_TEST (ID,NAME,AGE) VALUES (1,'Alice',25)");
        let _ = exec(&mut c, "INSERT INTO RUST_TEST (ID,NAME,AGE) VALUES (2,'Bob',30)");
        let _ = exec(&mut c, "INSERT INTO RUST_TEST (ID,NAME,AGE) VALUES (3,'Charlie',35)");
        Ok("3 rows".into())
    })(), &mut p, &mut f);
    run("SELECT *", (|| {
        let rs = c.execute("SELECT ID,NAME,AGE FROM RUST_TEST ORDER BY ID").map_err(|e| e.to_string())?;
        assert_eq!(rs.len(), 3);
        assert_eq!(rs.columns.len(), 3);
        assert_eq!(rs.columns[0].name, "ID");
        assert_eq!(rs.columns[1].name, "NAME");
        assert_eq!(rs.columns[2].name, "AGE");
        Ok(format!("{} rows {} cols", rs.len(), rs.columns.len()))
    })(), &mut p, &mut f);
    run("SELECT WHERE", (|| {
        let rs = c.execute("SELECT ID FROM RUST_TEST WHERE NAME='Bob'").map_err(|e| e.to_string())?;
        let v = g32(&rs)?; assert_eq!(v, "2");
        Ok("Bob ID=2".into())
    })(), &mut p, &mut f);
    run("UPDATE", (|| {
        let _ = exec(&mut c, "UPDATE RUST_TEST SET AGE=26 WHERE NAME='Alice'");
        let rs = c.execute("SELECT AGE FROM RUST_TEST WHERE NAME='Alice'").map_err(|e| e.to_string())?;
        let v = g32(&rs)?; assert_eq!(v, "26");
        Ok("Alice=26".into())
    })(), &mut p, &mut f);
    run("DELETE", (|| {
        let _ = exec(&mut c, "DELETE FROM RUST_TEST WHERE ID=3");
        let rs = c.execute("SELECT COUNT(*) FROM RUST_TEST").map_err(|e| e.to_string())?;
        let v = g32(&rs)?; assert_eq!(v, "2");
        Ok("2 remaining".into())
    })(), &mut p, &mut f);
    run("Multi-column", (|| {
        let rs = c.execute("SELECT ID,NAME FROM RUST_TEST WHERE ID=1").map_err(|e| e.to_string())?;
        assert_eq!(rs.columns.len(), 2);
        assert_eq!(rs.columns[0].type_code, 4);
        assert_eq!(rs.columns[1].type_code, 3);
        Ok("ok".into())
    })(), &mut p, &mut f);

    println!("\n=== Transaction Tests ===");
    run("ROLLBACK", (|| {
        let _ = exec(&mut c, "INSERT INTO RUST_TEST (ID,NAME,AGE) VALUES (100,'X',99)");
        c.rollback().map_err(|e| e.to_string())?;
        let rs = c.execute("SELECT COUNT(*) FROM RUST_TEST").map_err(|e| e.to_string())?;
        let v = g32(&rs)?; assert_eq!(v, "2");
        Ok("rollback ok".into())
    })(), &mut p, &mut f);
    run("DROP+COMMIT", (|| {
        let _ = exec(&mut c, "DROP TABLE RUST_TEST");
        c.commit().map(|_| "committed".into()).map_err(|e| e.to_string())
    })(), &mut p, &mut f);

    println!("\n=== Async Client Tests ===");
    async fn async_tests(host: String, port: u16, user: String, pass: String, p: &mut i32, f: &mut i32) {
        let mut ac = tokio_dameng::Client::new(&host, port);
        run("async connect", ac.connect(&user, &pass).await.map(|_| "ok".into()).map_err(|e| e.to_string()), p, f);
        {
            let rs = ac.execute("SELECT 1").await.map_err(|e| e.to_string()).unwrap();
            assert_eq!(rs.len(), 1);
            let val = rs.rows.first().unwrap().get_i32(0).unwrap();
            assert_eq!(val, 1);
            run("async SELECT 1", Ok(format!("got {}", val)), p, f);
        }
        {
            let rs = ac.execute("SELECT * FROM V$VERSION").await.map_err(|e| e.to_string()).unwrap();
            assert!(rs.len() > 0);
            run("async V$VERSION", Ok("ok".into()), p, f);
        }
        {
            let _ = aexec(&mut ac, "DROP TABLE IF EXISTS RUST_ASYNC").await;
            let _ = aexec(&mut ac, "CREATE TABLE RUST_ASYNC (ID INT PRIMARY KEY, NAME VARCHAR(100), SCORE FLOAT)").await;
            let _ = aexec(&mut ac, "INSERT INTO RUST_ASYNC (ID,NAME,SCORE) VALUES (1,'Neko',99.5)").await;
            let rs = ac.execute("SELECT ID,NAME,SCORE FROM RUST_ASYNC WHERE ID=1").await.unwrap();
            assert_eq!(rs.len(), 1);
            assert_eq!(rs.columns.len(), 3);
            assert_eq!(rs.rows.first().unwrap().get_i32(0).unwrap(), 1);
            run("async CRUD", Ok("ok".into()), p, f);
        }
        {
            use tokio_dameng::QueryBuilderExt;
            let rs = ac.query("SELECT 42 AS ANS").fetch_all().await.unwrap();
            assert_eq!(rs.len(), 1);
            run("async query API", Ok("ok".into()), p, f);
        }
        {
            let _ = aexec(&mut ac, "DROP TABLE RUST_ASYNC").await;
            ac.commit().await.map_err(|e| e.to_string()).unwrap();
            run("async cleanup", Ok("committed".into()), p, f);
        }
    }
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async_tests(host.clone(), port, user.clone(), pass.clone(), &mut p, &mut f));

    println!("\n=== Summary ===");
    println!("Passed: {}", p);
    println!("Failed: {}", f);
    if f > 0 { std::process::exit(1); }
}
