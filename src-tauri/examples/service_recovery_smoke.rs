//! Force-kill an isolated controller, then recover only its own web services.
#[cfg(unix)]
mod smoke {
    use fairway_core::{
        config::{Layout, Ports},
        manager::{Manager, Snapshot},
        process,
    };
    use std::{
        fs,
        net::TcpListener,
        path::{Path, PathBuf},
        process::Child,
        thread,
        time::{Duration, Instant},
    };

    fn layout(root: &Path, runtime: &Path) -> Layout {
        Layout::at(
            root.join("student's home ä"),
            root.join("data ä"),
            runtime.into(),
        )
    }

    fn start_controller(root: &Path, runtime: &Path) -> Result<(), String> {
        let mut manager = Manager::new(layout(root, runtime))?;
        let listeners = (0..6)
            .map(|_| TcpListener::bind(("127.0.0.1", 0)))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?;
        let ports = listeners
            .iter()
            .map(|l| l.local_addr().unwrap().port())
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
        manager.start("nginx")?;
        fs::write(
            root.join("ready.json"),
            serde_json::to_vec(&manager.snapshot()).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    struct Controller {
        child: Child,
        root: PathBuf,
        runtime: PathBuf,
    }

    impl Drop for Controller {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            if let Ok(mut manager) = Manager::new(layout(&self.root, &self.runtime)) {
                let _ = manager.stop_all();
            }
            // Also clean up when a failed recovery assertion exposed the bug.
            let layout = layout(&self.root, &self.runtime);
            for id in ["nginx", "php"] {
                let pid = fs::read_to_string(layout.data.join(format!("{id}.pid")))
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok());
                if let Some(pid) = pid.filter(|pid| process::matches_web(*pid, id, &layout)) {
                    let mut owned = process::OwnedProcess::adopt_database(pid);
                    let _ = owned.graceful_signal();
                    let _ = owned.wait(10);
                }
            }
        }
    }

    pub fn run() -> Result<(), String> {
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        if args.first().is_some_and(|arg| arg == "--controller") {
            return start_controller(Path::new(&args[1]), Path::new(&args[2]));
        }
        let runtime = args
            .first()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"));
        let root = std::env::temp_dir().join(format!(
            "fairway-web-recovery-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir(&root).map_err(|e| e.to_string())?;
        let child = process::command(std::env::current_exe().map_err(|e| e.to_string())?)
            .arg("--controller")
            .arg(&root)
            .arg(&runtime)
            .spawn()
            .map_err(|e| e.to_string())?;
        let mut controller = Controller {
            child,
            root: root.clone(),
            runtime: runtime.clone(),
        };
        let deadline = Instant::now() + Duration::from_secs(30);
        let before: Snapshot = loop {
            if let Ok(bytes) = fs::read(root.join("ready.json")) {
                break serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            }
            if controller
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
                || Instant::now() >= deadline
            {
                return Err(format!(
                    "Isolated controller failed to start; logs: {}",
                    root.display()
                ));
            }
            thread::sleep(Duration::from_millis(100));
        };
        controller.child.kill().map_err(|e| e.to_string())?;
        controller.child.wait().map_err(|e| e.to_string())?;
        let mut recovered = Manager::new(layout(&root, &runtime))?;
        let after = recovered.snapshot();
        for id in ["nginx", "php"] {
            let original = before.services.iter().find(|s| s.id == id).unwrap();
            let current = after.services.iter().find(|s| s.id == id).unwrap();
            assert!(current.running && current.ready, "{id} was not recovered");
            assert_eq!(
                current.pid, original.pid,
                "{id} was replaced instead of recovered"
            );
        }
        // Identical binaries with another configuration must not be adopted.
        let foreign = layout(&root.join("other project"), &runtime);
        foreign.initialize()?;
        for id in ["nginx", "php"] {
            fs::copy(
                recovered.layout.data.join(format!("{id}.pid")),
                foreign.data.join(format!("{id}.pid")),
            )
            .map_err(|e| e.to_string())?;
        }
        let mut foreign = Manager::new(foreign)?;
        assert!(foreign.snapshot().services.iter().all(|s| !s.running));
        foreign.stop_all()?;
        let stale = layout(&root.join("unrelated live pid"), &runtime);
        stale.initialize()?;
        for id in ["nginx", "php"] {
            fs::write(
                stale.data.join(format!("{id}.pid")),
                std::process::id().to_string(),
            )
            .map_err(|e| e.to_string())?;
        }
        let mut stale = Manager::new(stale)?;
        assert!(stale.snapshot().services.iter().all(|s| !s.running));
        stale.stop_all()?;
        assert!(recovered
            .snapshot()
            .services
            .iter()
            .filter(|s| s.id == "nginx" || s.id == "php")
            .all(|s| s.ready));
        recovered.stop("php")?;
        for service in before
            .services
            .iter()
            .filter(|s| s.id == "nginx" || s.id == "php")
        {
            assert!(
                !process::pid_alive(service.pid.unwrap()),
                "{} survived Stop",
                service.id
            );
        }
        recovered.start("nginx")?;
        assert!(recovered
            .snapshot()
            .services
            .iter()
            .filter(|s| s.id == "nginx" || s.id == "php")
            .all(|s| s.ready));
        recovered.stop_all()?;
        drop(controller);
        fs::remove_dir_all(root).map_err(|e| e.to_string())?;
        println!("PASS: force-killed controller recovered the same Nginx/PHP masters; foreign configuration and unrelated PID rejected; Stop/Start works.");
        Ok(())
    }
}

fn main() -> Result<(), String> {
    #[cfg(unix)]
    smoke::run()?;
    Ok(())
}
