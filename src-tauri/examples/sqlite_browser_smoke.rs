//! SQLite file browsing through the production native boundary, without services.
use fairway_core::{config::Layout, data_browser, manager::Manager};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

fn main() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| e.to_string())?.join(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass a fresh isolated test directory.")?,
    );
    if root.exists() {
        return Err("Use a fresh test directory.".into());
    }
    let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime");
    let base = Layout::at(
        root.join("Student May's home 日本"),
        root.join("data 日本"),
        runtime,
    );
    for version in base.php_versions()?.keys() {
        let manager = Manager::new(Layout::at(
            base.home.join(version),
            base.data.join(version),
            base.runtime.clone(),
        ))?;
        let layout = &manager.layout;
        fs::write(layout.data.join("php-version"), version).map_err(|e| e.to_string())?;
        layout.write_configs(&manager.ports)?;
        exercise(layout)?;
        println!("PASS: PHP {version} SQLite native worker in Unicode home/data paths.");
    }
    Ok(())
}

fn exercise(layout: &Layout) -> Result<(), String> {
    let path = layout.www.join("Student May's database.sqlite");
    let call = |mut r: Value| {
        r["driver"] = json!("sqlite");
        r["path"] = json!(path);
        data_browser::execute(layout, r, Arc::new(AtomicBool::new(false)))
    };
    assert!(call(json!({"op":"tables"}))
        .unwrap_err()
        .contains("missing"));
    assert!(!path.exists());
    fs::write(&path, b"not a database").map_err(|e| e.to_string())?;
    assert!(call(json!({"op":"tables"}))
        .unwrap_err()
        .contains("SQLite 3"));
    assert_eq!(fs::read(&path).unwrap(), b"not a database");
    fs::write(&path, b"").map_err(|e| e.to_string())?;
    assert_eq!(call(json!({"op":"tables"}))?["tables"], json!([]));
    call(
        json!({"op":"query","sql":"CREATE TABLE students(id INTEGER PRIMARY KEY, name TEXT); INSERT INTO students VALUES(9007199254740993,'May 日本');"}),
    )?;
    let page = call(json!({"op":"rows","table":"students"}))?;
    assert_eq!(page["rows"][0]["id"], "9007199254740993");
    let backup = call(json!({"op":"backup"}))?;
    let backup_path = backup["path"].as_str().unwrap();
    let copied = data_browser::execute(
        layout,
        json!({"driver":"sqlite","path":backup_path,"op":"rows","table":"students"}),
        Arc::new(AtomicBool::new(false)),
    )?;
    assert_eq!(copied["rows"], page["rows"]);
    assert!(call(json!({"op":"drop_table","table":"students"}))
        .unwrap_err()
        .contains("Confirm"));
    call(json!({"op":"drop_table","table":"students","confirmed":true}))?;
    assert_eq!(call(json!({"op":"tables"}))?["tables"], json!([]));
    assert_eq!(
        fs::read_dir(layout.data.join("backups")).unwrap().count(),
        2
    );
    call(json!({"op":"query","sql":"CREATE TABLE keep_me(id INTEGER PRIMARY KEY)"}))?;
    // An unavailable backup destination must prevent the destructive operation.
    let backups = layout.data.join("backups");
    let saved = layout.data.join("saved-backups");
    fs::rename(&backups, &saved).map_err(|e| e.to_string())?;
    fs::write(&backups, b"blocked").map_err(|e| e.to_string())?;
    assert!(call(json!({"op":"drop_table","table":"keep_me","confirmed":true})).is_err());
    assert_eq!(
        call(json!({"op":"tables"}))?["tables"][0]["name"],
        "keep_me"
    );
    fs::remove_file(&backups).map_err(|e| e.to_string())?;
    fs::rename(&saved, &backups).map_err(|e| e.to_string())?;
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let timer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        signal.store(true, Ordering::SeqCst);
    });
    let result = data_browser::execute(
        layout,
        json!({"driver":"sqlite","path":path,"op":"query","sql":"WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n) SELECT sum(x) FROM n"}),
        cancel,
    );
    timer.join().unwrap();
    assert!(result.unwrap_err().contains("canceled or timed out"));
    call(json!({"op":"insert","table":"keep_me","values":{"id":"1"}}))?;
    assert!(!layout.data.join("mysql.pid").exists());
    println!("SQLite native smoke passed: original files, exact rows, consistent backups, destructive guards, cancellation; no services started.");
    Ok(())
}
