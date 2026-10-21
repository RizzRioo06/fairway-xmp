//! Real-runtime checks shared by the focused and full service smoke examples.
use fairway_core::{
    config::{windows_ansi_path, Layout, Ports},
    manager::{Manager, ServiceStatus},
    process,
};
use std::{fs, net::TcpListener, path::Path, process::Command};

fn mysql(manager: &Manager, executable: &str, secure: bool) -> Command {
    let mut command = process::command(windows_ansi_path(&manager.layout.exe("mysql", executable)));
    if secure {
        command.arg(format!(
            "--defaults-file={}",
            windows_ansi_path(&manager.layout.data.join("config/admin.cnf")).display()
        ));
        command.arg("--no-login-paths");
    } else {
        command
            .args([
                "--no-defaults",
                "--no-login-paths",
                "--skip-password",
                "--user=root",
                "--host=127.0.0.1",
                "--protocol=TCP",
            ])
            .arg(format!("--port={}", manager.ports.mysql));
    }
    command.arg("--connect-timeout=3");
    command
}

fn status(manager: &mut Manager) -> ServiceStatus {
    manager
        .snapshot()
        .services
        .into_iter()
        .find(|service| service.id == "mysql")
        .unwrap()
}

pub fn run(root: &Path, runtime: &Path) -> Result<(), String> {
    let layout = Layout::at(
        root.join("student's home မြန်မာ"),
        root.join("student data"),
        runtime.into(),
    );
    let mut manager = Manager::new(layout)?;
    let listeners = (0..6)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|error| error.to_string())?;
    let ports = listeners
        .iter()
        .map(|listener| listener.local_addr().unwrap().port())
        .collect::<Vec<_>>();
    drop(listeners);
    manager.save_ports(Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
        mail: ports[4],
        smtp: ports[5],
    })?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<(), String> {
            manager.start("mysql")?;
            assert!(status(&mut manager).ready);
            let credentials = fs::read(manager.layout.data.join("credentials"))
                .map_err(|error| error.to_string())?;
            assert!(!manager.layout.data.join("tmp/mysql-init.sql").exists());
            let rejected = process::sql(mysql(&manager, "mysql", false), "SELECT 1;").unwrap_err();
            assert!(
                rejected.contains("Access denied"),
                "Fresh root must reject passwordless authentication: {rejected}"
            );
            process::sql(mysql(&manager, "mysql", true),
            "CREATE DATABASE startup_recovery; CREATE TABLE startup_recovery.coursework (id INT PRIMARY KEY); INSERT INTO startup_recovery.coursework VALUES (42);")?;
            let pid = status(&mut manager).pid;
            manager.leave_services_running()?;
            fs::remove_file(manager.layout.data.join("database-ready"))
                .map_err(|error| error.to_string())?;
            manager = Manager::new(manager.layout.clone())?;
            let recovered = status(&mut manager);
            assert!(
                recovered.running && !recovered.ready,
                "Recovered incomplete setup must remain pending"
            );
            assert_eq!(recovered.pid, pid);
            manager.start("mysql")?;
            assert!(status(&mut manager).ready);
            assert_eq!(
                status(&mut manager).pid,
                pid,
                "Retry must provision the recovered process"
            );

            // Model an old first run interrupted before root received its password.
            fs::remove_file(manager.layout.data.join("database-ready"))
                .map_err(|error| error.to_string())?;
            process::sql(
                mysql(&manager, "mysql", true),
                "ALTER USER 'root'@'localhost' IDENTIFIED BY '';",
            )?;
            manager.stop("mysql")?;
            assert!(!status(&mut manager).running);
            assert!(!process::connected(manager.ports.mysql));
            manager.start("mysql")?;
            assert!(status(&mut manager).ready);
            assert!(process::sql(mysql(&manager, "mysql", false), "SELECT 1;")
                .unwrap_err()
                .contains("Access denied"));
            let mut query = mysql(&manager, "mysql", true);
            query.args([
                "--batch",
                "--skip-column-names",
                "-e",
                "SELECT id FROM startup_recovery.coursework;",
            ]);
            assert_eq!(process::output(query)?, "42");
            assert_eq!(
                fs::read(manager.layout.data.join("credentials"))
                    .map_err(|error| error.to_string())?,
                credentials
            );

            // A completed database must never be reset or shut down passwordlessly
            // just because its stored credentials no longer authenticate.
            process::sql(
                mysql(&manager, "mysql", true),
                "ALTER USER 'root'@'localhost' IDENTIFIED BY '';",
            )?;
            assert!(manager.stop("mysql").is_err());
            assert!(status(&mut manager).running);
            process::sql(
                mysql(&manager, "mysql", false),
                &format!(
                    "ALTER USER 'root'@'localhost' IDENTIFIED BY '{}';",
                    manager.layout.secret()?
                ),
            )?;
            manager.stop("mysql")?;
            Ok(())
        },
    ));
    // Even a failed assertion must clean up this disposable server gracefully.
    if process::connected(manager.ports.mysql) {
        let _ = process::sql(
            mysql(&manager, "mysql", false),
            &format!(
                "ALTER USER 'root'@'localhost' IDENTIFIED BY '{}';",
                manager.layout.secret()?
            ),
        );
        let mut shutdown = mysql(&manager, "mysqladmin", true);
        shutdown.arg("shutdown");
        process::output(shutdown)?;
    }
    let shutdown = manager.stop_all();
    result
        .map_err(|_| "MySQL startup/recovery assertion failed.".to_string())?
        .and(shutdown)?;
    println!("PASS: secure MySQL bootstrap, passwordless rejection, pending-process recovery, legacy graceful shutdown, preserved credentials/coursework, and provisioned authentication failure.");
    Ok(())
}
