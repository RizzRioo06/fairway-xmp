//! Headless fixture worker. Input credentials arrive only on private stdin.
fn main() {
    if fairway_core::postgresql::explorer::worker_entrypoint() {
        return;
    }
    #[cfg(windows)]
    if run().is_err() {
        eprintln!("Windows PostgreSQL fixture failed; credentials and server diagnostics withheld");
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!("This fixture requires Windows x64");
        std::process::exit(1);
    }
}
#[cfg(any(windows, test))]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use fairway_core::{
        config::Layout,
        postgresql::{explorer::Explorer, explorer_contract::*, Managed},
    };
    use std::{
        io::Read,
        path::PathBuf,
        sync::Arc,
        time::{Duration, Instant},
    };
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Input {
        root: PathBuf,
        runtime: PathBuf,
        connection: Connect,
    }
    let mut bytes = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut bytes)?;
    if bytes.len() > 65536 {
        return Err("Fixture input too large".into());
    }
    let input: Input = serde_json::from_slice(&bytes)?;
    for b in &mut bytes {
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    if !input.root.is_absolute() || !input.runtime.is_absolute() || input.root.exists() {
        return Err("Expected a fresh absolute fixture root and packaged runtime".into());
    }
    let (host, port, user) = match &input.connection.target {
        Target::External {
            host, port, user, ..
        } => (host.clone(), *port, user.clone()),
        _ => return Err("External fixture only".into()),
    };
    std::fs::create_dir(&input.root)?;
    let layout = Layout {
        home: input.root.join("home"),
        data: input.root.join("data"),
        runtime: input.runtime,
        www: input.root.join("www"),
        bin: input.root.join("bin"),
    };
    std::fs::create_dir(&layout.home)?;
    std::fs::create_dir(&layout.data)?;
    let managed = Arc::new(Managed::new(Some(layout.clone())));
    let explorer = Arc::new(Explorer::default());
    let session = explorer
        .connect(input.connection)
        .map_err(|_| "Descriptor open failed")?;
    let execute = |operation: Operation| {
        explorer.execute(
            &managed,
            Request {
                job_id: format!("{:032x}", rand::random::<u128>()),
                session_id: session.id.clone(),
                operation,
            },
        )
    };
    let query = |sql: &str, write: bool| Operation::Query {
        sql: sql.into(),
        parameters: vec![],
        mode: if write {
            QueryMode::ReadWrite
        } else {
            QueryMode::ReadOnly
        },
        confirmed: true,
    };
    let schemas = execute(Operation::Schemas).map_err(|_| "Schema request failed")?;
    assert!(schemas
        .rows
        .iter()
        .any(|row| row.iter().any(|v| v.as_deref() == Some("public"))));
    assert!(execute(query("CREATE TABLE public.windows_fixture (id bigint PRIMARY KEY, payload text, note text, blank text)",false)).is_err());
    let create=execute(query("CREATE TABLE public.windows_fixture (id bigint PRIMARY KEY, payload text, note text, blank text)",true)).map_err(|_|"Authorized create failed")?;
    assert_eq!(create.outcome, Outcome::Committed);
    execute(query(
        "INSERT INTO public.windows_fixture VALUES (9007199254740993, '日本', NULL, '')",
        true,
    ))
    .map_err(|_| "Authorized insert failed")?;
    let relations = execute(Operation::Relations {
        schema: "public".into(),
    })
    .map_err(|_| "Relations failed")?;
    assert!(relations
        .rows
        .iter()
        .any(|row| row.iter().any(|v| v.as_deref() == Some("windows_fixture"))));
    let rows = execute(Operation::Rows {
        schema: "public".into(),
        relation: "windows_fixture".into(),
        page: Page::default(),
    })
    .map_err(|_| "Rows failed")?;
    assert_eq!(
        rows.rows,
        vec![vec![
            Some("9007199254740993".into()),
            Some("日本".into()),
            None,
            Some("".into())
        ]]
    );
    execute(Operation::Columns {
        schema: "public".into(),
        relation: "windows_fixture".into(),
    })
    .map_err(|_| "Columns failed")?;
    execute(Operation::Indexes {
        schema: "public".into(),
        relation: "windows_fixture".into(),
    })
    .map_err(|_| "Indexes failed")?;
    let filtered = execute(Operation::Rows {
        schema: "public".into(),
        relation: "windows_fixture".into(),
        page: Page {
            limit: 50,
            offset: 0,
            sort: vec![Sort {
                column: "id".into(),
                direction: Direction::Desc,
            }],
            filters: vec![Filter {
                column: "payload".into(),
                operator: FilterOperator::Eq,
                value: Some("日本".into()),
            }],
        },
    })
    .map_err(|_| "Bound filter failed")?;
    assert_eq!(filtered.rows, rows.rows);
    let make_target = |transport: Transport| -> Target {
        serde_json::from_value(serde_json::json!({"kind":"external","host":host,"port":port,"user":user,"password":"wrong-fixture-password","transport":match transport{Transport::Loopback=>"loopback",Transport::VerifyFull=>"verifyFull"}})).unwrap()
    };
    let bad = explorer
        .connect(Connect {
            confirmed: true,
            database: session.database.clone(),
            target: make_target(Transport::Loopback),
        })
        .map_err(|_| "Bad-password descriptor failed")?;
    assert_eq!(
        explorer
            .execute(
                &managed,
                Request {
                    job_id: format!("{:032x}", rand::random::<u128>()),
                    session_id: bad.id.clone(),
                    operation: Operation::Schemas
                }
            )
            .unwrap_err()
            .code,
        Code::ConnectionFailed
    );
    explorer
        .disconnect(SessionRequest { session_id: bad.id })
        .map_err(|_| "Disconnect failed")?;
    assert_eq!(
        explorer
            .connect(Connect {
                confirmed: true,
                database: session.database.clone(),
                target: make_target(Transport::VerifyFull)
            })
            .unwrap_err()
            .code,
        Code::Unavailable
    );
    let job = format!("{:032x}", rand::random::<u128>());
    let owner = explorer.clone();
    let service = managed.clone();
    let id = session.id.clone();
    let started = job.clone();
    let task = std::thread::spawn(move || {
        owner.execute(
            &service,
            Request {
                job_id: started,
                session_id: id,
                operation: Operation::Query {
                    sql: "SELECT pg_catalog.pg_sleep(30)".into(),
                    parameters: vec![],
                    mode: QueryMode::ReadOnly,
                    confirmed: true,
                },
            },
        )
    });
    let active="SELECT count(*)::text FROM pg_catalog.pg_stat_activity WHERE application_name='fairway-postgresql-job' AND pid<>pg_catalog.pg_backend_pid() AND state='active' AND query LIKE '%pg_sleep(30)%'";
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut observed = false;
    while Instant::now() < deadline {
        let result = execute(query(active, false)).map_err(|_| "Activity observer failed")?;
        if result
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(|v| v.as_deref())
            == Some("1")
        {
            observed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    explorer
        .cancel(Cancel { job_id: job })
        .map_err(|_| "Cancel dispatch failed")?;
    let result = task.join().map_err(|_| "Query thread failed")?;
    assert!(observed, "Query was not observed active");
    assert_eq!(result.unwrap_err().code, Code::Cancelled);
    let result = execute(query(active, false)).map_err(|_| "Post-cancel observer failed")?;
    assert_eq!(result.rows[0][0].as_deref(), Some("0"));
    // Metadata integrity is checked before any DLL is loaded or server contacted.
    let shadow = input.root.join("tampered-runtime");
    std::fs::create_dir_all(shadow.join("php/bin"))?;
    for entry in std::fs::read_dir(layout.runtime.join("php/bin"))? {
        let entry = entry?;
        if entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("dll"))
        {
            std::fs::copy(entry.path(), shadow.join("php/bin").join(entry.file_name()))?;
        }
    }
    use std::io::Write;
    std::fs::OpenOptions::new()
        .append(true)
        .open(shadow.join("php/bin/libpq.dll"))?
        .write_all(b"tampered")?;
    let tampered = Managed::new(Some(Layout {
        runtime: shadow,
        ..layout
    }));
    assert_eq!(
        explorer
            .execute(
                &tampered,
                Request {
                    job_id: format!("{:032x}", rand::random::<u128>()),
                    session_id: session.id.clone(),
                    operation: Operation::Schemas
                }
            )
            .unwrap_err()
            .code,
        Code::Unavailable
    );
    explorer
        .disconnect(SessionRequest {
            session_id: session.id,
        })
        .map_err(|_| "Disconnect failed")?;
    println!("Windows PostgreSQL: exact values, catalogs, filters, SQL consent, authentication, observed cancellation and DLL tamper refusal passed");
    Ok(())
}

#[cfg(test)]
#[test]
fn fixture_contract_typechecks_without_running_a_server() {
    let _: fn() -> Result<(), Box<dyn std::error::Error>> = run;
}
