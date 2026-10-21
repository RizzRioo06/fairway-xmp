//! Row-edit regressions against disposable bundled MySQL/PHP, never installed data.
use fairway_core::{
    config::{Layout, Ports},
    data_browser,
    manager::Manager,
};
use serde_json::{json, Value};
use std::{
    net::TcpListener,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
fn call(layout: &Layout, request: Value) -> Result<Value, String> {
    data_browser::execute(layout, request, Arc::new(AtomicBool::new(false)))
}
fn main() -> Result<(), String> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Provide a fresh isolated directory.")?,
    );
    if root.join("data").exists() {
        return Err("Use a fresh test directory; existing data was kept.".into());
    }
    let reproduce = std::env::args().any(|arg| arg == "--reproduce");
    let layout = Layout::at(
        root.join("home"),
        root.join("data"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"),
    );
    let mut manager = Manager::new(layout)?;
    let sockets: Vec<_> = (0..6)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)).unwrap())
        .collect();
    let ports: Vec<_> = sockets
        .iter()
        .map(|s| s.local_addr().unwrap().port())
        .collect();
    manager.save_ports(Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
        mail: ports[4],
        smtp: ports[5],
    })?;
    drop(sockets);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<(), String> {
            manager.start("mysql")?;
            let l = &manager.layout;
            call(
                l,
                json!({"op":"create_database","database":"row_regressions"}),
            )?;
            call(
                l,
                json!({"op":"query","database":"row_regressions","sql":"CREATE TABLE latin_values (id INT PRIMARY KEY, label VARCHAR(100) CHARACTER SET latin1, note TEXT NULL); INSERT INTO latin_values VALUES (1,'café',NULL); CREATE TABLE bits (id INT PRIMARY KEY, b BIT(3), wide BIT(9), note TEXT); INSERT INTO bits VALUES (1,b'101',b'100000001','kept'); CREATE TABLE doubles (id INT PRIMARY KEY, d DOUBLE, note TEXT); INSERT INTO doubles VALUES (1,1.2345678901234567,'kept'); CREATE TABLE large_values (id INT PRIMARY KEY, body LONGTEXT, note TEXT); INSERT INTO large_values VALUES (1,REPEAT('x',4*1024*1024),'kept');"}),
            )?;
            for table in ["latin_values", "bits", "doubles", "large_values"] {
                let page = call(
                    l,
                    json!({"op":"rows","database":"row_regressions","table":table}),
                )?;
                if table == "doubles" {
                    println!("DOUBLE displayed: {}", page["rows"][0]["d"]);
                }
                if table == "bits" {
                    println!(
                        "BIT display: {} / {}",
                        page["rows"][0]["b"], page["rows"][0]["wide"]
                    );
                }
                let mut request = json!({"op":"update","database":"row_regressions","table":table,"original":page["rows"][0],"values":{"note":"changed"}});
                if let Some(reference) = page["rowRefs"].get(0) {
                    request["rowRef"] = reference.clone();
                    request.as_object_mut().unwrap().remove("original");
                }
                let result = call(l, request);
                if reproduce {
                    println!(
                        "{table}: {}",
                        result
                            .as_ref()
                            .map(|_| "saved".to_string())
                            .unwrap_or_else(|e| e.clone())
                    );
                } else {
                    result?;
                }
            }
            if !reproduce {
                regressions(l)?;
                // The selected PHP version is independent of MySQL. Recheck exact
                // representations and a full chunk through every bundled driver.
                for version in l.php_versions()?.keys() {
                    fairway_core::config::atomic_write(
                        &l.data.join("php-version"),
                        version.as_bytes(),
                    )?;
                    let page = load_page(l, "bits")?;
                    assert_eq!(page["rows"][0]["b"], "0x05");
                    update(l, "bits", &page["rowRefs"][0], json!({}))?;
                    let floats = load_page(l, "float_values")?;
                    assert_eq!(floats["rows"][0]["id"], "1.2345677614212036");
                    update(l, "float_values", &floats["rowRefs"][0], json!({}))?;
                    let page = load_page(l, "doubles")?;
                    assert_eq!(page["rows"][0]["d"], "1.2345678901234567");
                    update(l, "doubles", &page["rowRefs"][0], json!({"note":"changed"}))?;
                    let page = load_page(l, "large_values")?;
                    let chunk = cell(l, "large_values", &page["rowRefs"][0], "body", 0)?;
                    assert_eq!(chunk["value"].as_str().unwrap().chars().count(), 131072);
                    println!(
                        "PHP {version}: exact values, no-op saves and bounded cell reads passed"
                    );
                }
            }
            let unicode = "日本".repeat(16) + &"a".repeat(32);
            let result = call(l, json!({"op":"create_database","database":unicode}));
            if reproduce {
                println!(
                    "64-character Unicode identifier: {}",
                    result
                        .as_ref()
                        .map(|_| "created".to_string())
                        .unwrap_or_else(|e| e.clone())
                );
            } else {
                result?;
            }
            Ok(())
        },
    ));
    manager.stop_all()?;
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn load_page(l: &Layout, table: &str) -> Result<Value, String> {
    call(
        l,
        json!({"op":"rows","database":"row_regressions","table":table}),
    )
}
fn sql(l: &Layout, sql: &str) -> Result<Value, String> {
    call(
        l,
        json!({"op":"query","database":"row_regressions","sql":sql}),
    )
}
fn update(l: &Layout, table: &str, reference: &Value, values: Value) -> Result<Value, String> {
    call(
        l,
        json!({"op":"update","database":"row_regressions","table":table,"rowRef":reference,"values":values}),
    )
}
fn cell(
    l: &Layout,
    table: &str,
    reference: &Value,
    column: &str,
    offset: u64,
) -> Result<Value, String> {
    call(
        l,
        json!({"op":"cell","database":"row_regressions","table":table,"rowRef":reference,"column":column,"offset":offset}),
    )
}
fn regressions(l: &Layout) -> Result<(), String> {
    sql(l, "CREATE TABLE float_values(id FLOAT PRIMARY KEY, f FLOAT, note TEXT); INSERT INTO float_values VALUES(1.2345678,1.2345678,'kept')")?;
    let floats = load_page(l, "float_values")?;
    println!(
        "FLOAT key and value: {} / {}",
        floats["rows"][0]["id"], floats["rows"][0]["f"]
    );
    update(
        l,
        "float_values",
        &floats["rowRefs"][0],
        json!({"note":"changed"}),
    )?;
    let floats = load_page(l, "float_values")?;
    assert_eq!(floats["rows"][0]["f"], "1.2345677614212036");
    sql(l, "UPDATE float_values SET f=1.2345679")?;
    let changed = load_page(l, "float_values")?;
    assert_ne!(
        changed["rowRefs"][0]["version"],
        floats["rowRefs"][0]["version"]
    );
    assert!(update(
        l,
        "float_values",
        &floats["rowRefs"][0],
        json!({"note":"stale"})
    )
    .is_err());
    let large = load_page(l, "large_values")?;
    let reference = &large["rowRefs"][0];
    assert_eq!(large["truncated"][0]["body"]["length"], 4 * 1024 * 1024);
    assert!(serde_json::to_vec(&large).unwrap().len() < 16000);
    update(l, "large_values", reference, json!({}))?;
    let mut offset = 0;
    loop {
        let part = cell(l, "large_values", reference, "body", offset)?;
        assert!(part["value"].as_str().unwrap().bytes().all(|b| b == b'x'));
        offset = part["nextOffset"].as_u64().unwrap();
        if part["complete"] == true {
            break;
        }
    }
    assert_eq!(offset, 4 * 1024 * 1024);
    // Same preview, different tail: both reads and writes must reject it.
    sql(
        l,
        "UPDATE large_values SET body=CONCAT(LEFT(body,CHAR_LENGTH(body)-1),'y')",
    )?;
    assert!(update(l, "large_values", reference, json!({})).is_err());
    assert!(update(l, "large_values", reference, json!({"note":"stale"})).is_err());
    assert!(cell(l, "large_values", reference, "body", offset - 131072).is_err());
    assert!(call(l, json!({"op":"delete","database":"row_regressions","table":"large_values","rowRef":reference})).is_err());
    // Large changed values are staged once and cleaned up even on failures.
    let text = "🟢é\n'\\".repeat(400000);
    let current = load_page(l, "large_values")?;
    update(
        l,
        "large_values",
        &current["rowRefs"][0],
        json!({"body":text}),
    )?;
    let written = load_page(l, "large_values")?;
    let mut actual = String::new();
    let mut offset = 0;
    loop {
        let part = cell(l, "large_values", &written["rowRefs"][0], "body", offset)?;
        actual.push_str(part["value"].as_str().unwrap());
        offset = part["nextOffset"].as_u64().unwrap();
        if part["complete"] == true {
            break;
        }
    }
    assert_eq!(actual, text);
    assert!(update(l, "large_values", reference, json!({"body":text})).is_err());
    assert!(!std::fs::read_dir(l.data.join("tmp"))
        .map_err(|e| e.to_string())?
        .any(|f| f
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("database-value-")));
    assert!(call(l,json!({"op":"update","database":"row_regressions","table":"large_values","rowRef":written["rowRefs"][0],"valueFiles":{"body":"/etc/passwd"}})).is_err());
    let latin = load_page(l, "latin_values")?;
    assert_eq!(latin["rows"][0]["label"], "café");
    update(l, "latin_values", &latin["rowRefs"][0], json!({"note":""}))?;
    let empty = load_page(l, "latin_values")?;
    assert_eq!(empty["rows"][0]["note"], "");
    update(
        l,
        "latin_values",
        &empty["rowRefs"][0],
        json!({"note":null}),
    )?;
    assert!(load_page(l, "latin_values")?["rows"][0]["note"].is_null());
    let latin = load_page(l, "latin_values")?;
    sql(l, "UPDATE latin_values SET label=UPPER(label),note='' ")?;
    assert!(update(l, "latin_values", &latin["rowRefs"][0], json!({})).is_err());
    let bits = load_page(l, "bits")?;
    assert!(update(l, "bits", &bits["rowRefs"][0], json!({"b":"0x07"})).is_err());
    sql(l, "UPDATE bits SET b=b'110'")?;
    assert!(update(l, "bits", &bits["rowRefs"][0], json!({"note":"stale"})).is_err());
    sql(l, "UPDATE bits SET b=b'101'; CREATE TABLE composite_keys (code VARCHAR(20) COLLATE utf8mb4_bin NOT NULL, seq INT NOT NULL, note TEXT, UNIQUE(code,seq)); INSERT INTO composite_keys VALUES ('A',1,'one'),('a',1,'two'); CREATE TABLE unkeyed (body LONGTEXT); INSERT INTO unkeyed VALUES(REPEAT('z',2000)),(REPEAT('z',2000)); CREATE VIEW row_view AS SELECT * FROM large_values; CREATE TABLE locks (id INT PRIMARY KEY, note TEXT); INSERT INTO locks VALUES(1,'one'),(2,'two');")?;
    let composite = load_page(l, "composite_keys")?;
    assert_eq!(composite["key"].as_array().unwrap().len(), 2);
    update(
        l,
        "composite_keys",
        &composite["rowRefs"][0],
        json!({"code":"B","note":"changed"}),
    )?;
    assert!(update(l, "composite_keys", &composite["rowRefs"][0], json!({})).is_err());
    assert_eq!(
        sql(l, "SELECT note FROM composite_keys WHERE code='a'")?["sets"][0]["rows"][0][0],
        "two"
    );
    let schema = load_page(l, "composite_keys")?;
    sql(l, "ALTER TABLE composite_keys ADD extra TEXT")?;
    assert!(update(l, "composite_keys", &schema["rowRefs"][0], json!({})).is_err());
    for table in ["unkeyed", "row_view"] {
        let page = load_page(l, table)?;
        assert!(page["key"].as_array().unwrap().is_empty());
        assert!(
            cell(l, table, &page["rowRefs"][0], "body", 0)?["value"]
                .as_str()
                .unwrap()
                .len()
                > 1000
        );
        assert!(update(l, table, &page["rowRefs"][0], json!({"body":"bad"})).is_err());
    }
    unrelated_lock(l)?;
    let current = load_page(l, "locks")?;
    call(
        l,
        json!({"op":"delete","database":"row_regressions","table":"locks","rowRef":current["rowRefs"][0]}),
    )?;
    assert_eq!(load_page(l, "locks")?["total"], "1");
    println!("Full-value, large-edit, stale-tail, NULL, binary, schema, composite-key, view and row-lock regressions passed");
    Ok(())
}
fn unrelated_lock(l: &Layout) -> Result<(), String> {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    struct Stop(std::process::Child);
    impl Drop for Stop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let original = load_page(l, "locks")?;
    let child = Command::new(l.exe("mysql","mysql"))
        .arg(format!("--defaults-file={}",l.data.join("config/admin.cnf").display()))
        .args(["--no-login-paths","--unbuffered","--batch","--skip-column-names","row_regressions","-e","BEGIN; SELECT id FROM locks WHERE id=2 FOR UPDATE; SELECT 'LOCKED'; DO SLEEP(10); COMMIT;"])
        .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    let mut child = Stop(child);
    let output = child.0.stdout.take().unwrap();
    let mut reader = BufReader::new(output);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("Lock fixture failed to start.".into());
        }
        if line.trim() == "LOCKED" {
            break;
        }
    }
    let start = Instant::now();
    update(
        l,
        "locks",
        &original["rowRefs"][0],
        json!({"note":"point update"}),
    )?;
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "An unrelated row lock blocked the indexed update"
    );
    // The locked row still waits and times out, keeping its original value.
    assert!(update(
        l,
        "locks",
        &original["rowRefs"][1],
        json!({"note":"must wait"})
    )
    .is_err());
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let result = std::thread::scope(|scope| {
        scope.spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            trigger.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        data_browser::execute(
            l,
            json!({"op":"update","database":"row_regressions","table":"locks","rowRef":original["rowRefs"][1],"values":{"note":"z".repeat(200000)}}),
            cancel,
        )
    });
    assert!(result.unwrap_err().to_lowercase().contains("cancel"));
    assert!(!std::fs::read_dir(l.data.join("tmp"))
        .map_err(|e| e.to_string())?
        .any(|f| f
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("database-value-")));
    drop(child);
    assert_eq!(load_page(l, "locks")?["rows"][1]["note"], "two");
    Ok(())
}
