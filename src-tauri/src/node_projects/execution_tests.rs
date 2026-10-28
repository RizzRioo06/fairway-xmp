use super::*;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
    state: NodeProjects,
    projects: Vec<ProjectConfig>,
    runner: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let runner = PathBuf::from(
            std::env::var_os("FAIRWAY_TEST_NODE_RUNNER")
                .expect("Build the node_projects_runner example and set FAIRWAY_TEST_NODE_RUNNER"),
        );
        let nodes: Vec<_> = std::env::split_paths(
            &std::env::var_os("FAIRWAY_TEST_NODE_RUNTIMES")
                .expect("Set two controlled test runtimes"),
        )
        .collect();
        assert_eq!(nodes.len(), 2);
        let root =
            std::env::temp_dir().join(format!("fw-node-run-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(root.join("home")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        let state = NodeProjects::new(Some(layout.clone()));
        state.executions.test_runner(runner.clone());
        let mut projects = Vec::new();
        for (index, node) in nodes.into_iter().enumerate() {
            let node = fs::canonicalize(node).unwrap();
            let output = Command::new(&node)
                .env_remove("NODE_OPTIONS")
                .env_remove("NODE_PATH")
                .args(["-p", "process.version"])
                .output()
                .unwrap();
            assert!(output.status.success());
            let version = String::from_utf8(output.stdout).unwrap().trim().to_owned();
            let directory = root.join(format!("project-{index} 日本"));
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("package.json"), br#"{"scripts":{"dev":"node server.cjs","stubborn":"node stubborn.cjs","exit":"node exit.cjs","fail":"node fail.cjs","flood":"node flood.cjs"}}"#).unwrap();
            fs::write(directory.join("server.cjs"), r#"const fs=require('fs');const {spawn}=require('child_process');const child=spawn(process.execPath,['child.cjs'],{stdio:'inherit'});fs.writeFileSync('pids.json',JSON.stringify([process.pid,child.pid]));console.log('READY '+process.version+' café 日本');console.error('stderr\nmultiline');setInterval(()=>{},1000);setTimeout(()=>process.exit(0),30000);"#).unwrap();
            fs::write(
                directory.join("stubborn.cjs"),
                r#"process.on('SIGTERM',()=>{});require('./server.cjs');"#,
            )
            .unwrap();
            fs::write(directory.join("child.cjs"), "process.on('SIGTERM',()=>{});setInterval(()=>{},1000);setTimeout(()=>process.exit(0),30000);").unwrap();
            fs::write(directory.join("exit.cjs"), "console.log('immediate exit');").unwrap();
            fs::write(
                directory.join("fail.cjs"),
                "console.error('controlled failure');process.exit(7);",
            )
            .unwrap();
            fs::write(directory.join("flood.cjs"), r#"process.stdout.write('x'.repeat(12*1024*1024),()=>process.stderr.write('日本\nend\n'));"#).unwrap();
            projects.push(ProjectConfig {
                id: format!("{index:032x}"),
                name: format!("Fixture {index}"),
                root: directory.to_str().unwrap().into(),
                script: "dev".into(),
                runtime: ProjectRuntime {
                    node_path: node.to_str().unwrap().into(),
                    version,
                },
                open_url: None,
            });
        }
        store::save(
            &layout.data,
            &store::load(&layout.data).unwrap().1,
            &store::Document {
                schema_version: 1,
                projects: projects.clone(),
            },
        )
        .unwrap();
        Self {
            root,
            state,
            projects,
            runner,
        }
    }
    fn request(&self, index: usize) -> StartRequest {
        StartRequest {
            id: self.projects[index].id.clone(),
            expected_revision: self.state.snapshot().revision.unwrap(),
            manifest_revision: manifest::inspect(&self.projects[index].root)
                .unwrap()
                .manifest_revision,
            confirmed: true,
        }
    }
    fn script(&self, index: usize, script: &str) {
        let data = self.root.join("data");
        let (mut document, revision) = store::load(&data).unwrap();
        document.projects[index].script = script.into();
        store::save(&data, &revision, &document).unwrap();
    }
    fn wait(&self, index: usize, predicate: impl Fn(&RunStatus) -> bool) -> RunStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(run) = self
                .state
                .runs()
                .runs
                .into_iter()
                .find(|run| run.project_id == self.projects[index].id)
            {
                if predicate(&run) {
                    return run;
                }
            }
            assert!(
                Instant::now() < deadline,
                "Run did not settle: {:?}",
                self.state.runs().runs
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn pids(&self, index: usize) -> Vec<i32> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(bytes) = fs::read(Path::new(&self.projects[index].root).join("pids.json")) {
                if let Ok(pids) = serde_json::from_slice(&bytes) {
                    return pids;
                }
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.state.stop_all_runs();
        self.state.disconnect_runs();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn dead(pid: i32) -> bool {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let count = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    count != size || unsafe { info.assume_init() }.pbi_status == 5
}
fn all_dead(pids: &[i32]) {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !pids.iter().all(|pid| dead(*pid)) {
        assert!(
            Instant::now() < deadline,
            "Controlled descendants survived cleanup: {pids:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
#[ignore = "Requires the controlled runner example and two real runtimes"]
fn owned_start_restart_concurrency_stale_ids_and_runtime_leases() {
    let fixture = Fixture::new();
    let before = fs::read(fixture.root.join("data/node-projects.json")).unwrap();
    let mut unconfirmed = fixture.request(0);
    unconfirmed.confirmed = false;
    assert_eq!(
        fixture.state.start(unconfirmed).unwrap_err().code,
        ErrorCode::ConfirmationRequired
    );
    let first = fixture.state.start(fixture.request(0)).unwrap();
    assert_eq!(first.state, RunState::Starting);
    assert_eq!(
        fixture
            .state
            .ensure_runtime_unused(Path::new(&fixture.projects[0].runtime.node_path))
            .unwrap_err()
            .code,
        ErrorCode::ProjectBusy
    );
    assert_eq!(
        fixture.state.start(fixture.request(0)).unwrap_err().code,
        ErrorCode::ProjectBusy
    );
    fixture.state.start(fixture.request(1)).unwrap();
    fixture.wait(0, |r| r.state == RunState::Running);
    fixture.wait(1, |r| r.state == RunState::Running);
    let pids = fixture.pids(0);
    let other_pids = fixture.pids(1);
    assert_ne!(
        fixture.projects[0].runtime.version,
        fixture.projects[1].runtime.version
    );
    // Verify the executed scripts see their own selected runtime, not merely stored metadata.
    for (index, project) in fixture.projects.iter().enumerate() {
        let run = fixture.wait(index, |run| run.state == RunState::Running);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let logs = fixture
                .state
                .logs(LogRequest {
                    id: project.id.clone(),
                    run_id: run.run_id.clone(),
                    after_cursor: 0,
                })
                .unwrap();
            let text: String = logs
                .records
                .iter()
                .filter(|r| r.stream == LogStream::Stdout)
                .map(|r| r.text.as_str())
                .collect();
            if text.contains(&format!("READY {} café 日本", project.runtime.version)) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Selected runtime output missing: {text}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    assert_eq!(
        fixture
            .state
            .restart(RestartRequest {
                id: first.project_id.clone(),
                run_id: "f".repeat(32),
                expected_revision: fixture.state.snapshot().revision.unwrap(),
                manifest_revision: fixture.request(0).manifest_revision,
                confirmed: true,
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleRun
    );
    assert_eq!(
        fixture
            .state
            .remove(RemoveRequest {
                id: first.project_id.clone(),
                expected_revision: fixture.state.snapshot().revision.unwrap()
            })
            .unwrap_err()
            .code,
        ErrorCode::ProjectBusy
    );
    assert_eq!(
        fixture
            .state
            .stop(StopRequest {
                id: first.project_id.clone(),
                run_id: "f".repeat(32)
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleRun
    );
    // No renderer is attached; a hidden/destroyed view cannot own these runs.
    thread::sleep(Duration::from_millis(250));
    assert!(!dead(other_pids[0]));
    let next = fixture
        .state
        .restart(RestartRequest {
            id: first.project_id.clone(),
            run_id: first.run_id.clone(),
            expected_revision: fixture.state.snapshot().revision.unwrap(),
            manifest_revision: fixture.request(0).manifest_revision,
            confirmed: true,
        })
        .unwrap();
    assert_ne!(first.run_id, next.run_id);
    all_dead(&pids);
    assert_eq!(
        fixture
            .state
            .stop(StopRequest {
                id: first.project_id.clone(),
                run_id: first.run_id
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleRun
    );
    fixture.wait(0, |r| r.state == RunState::Running);
    assert!(!dead(other_pids[0]));
    fixture.state.stop_all_runs().unwrap();
    all_dead(&other_pids);
    fixture
        .state
        .ensure_runtime_unused(Path::new(&fixture.projects[0].runtime.node_path))
        .unwrap();
    assert_eq!(
        fs::read(fixture.root.join("data/node-projects.json")).unwrap(),
        before
    );
}

#[test]
#[ignore = "Requires the controlled runner example and two real runtimes"]
fn owned_exit_failure_flood_unicode_and_persisted_logs() {
    let fixture = Fixture::new();
    fixture.script(0, "exit");
    fixture.state.start(fixture.request(0)).unwrap();
    let exited = fixture.wait(0, |r| r.state == RunState::Exited);
    assert_eq!(exited.exit_code, Some(0));
    fixture.script(0, "fail");
    fixture.state.start(fixture.request(0)).unwrap();
    let failed = fixture.wait(0, |r| r.state == RunState::Failed);
    assert_eq!(failed.exit_code, Some(7));
    fixture.script(0, "flood");
    fixture.state.start(fixture.request(0)).unwrap();
    let flood = fixture.wait(0, |r| r.state == RunState::Exited);
    let mut cursor = 0;
    let mut found_stderr = false;
    let mut gap = false;
    loop {
        let batch = fixture
            .state
            .logs(LogRequest {
                id: flood.project_id.clone(),
                run_id: flood.run_id.clone(),
                after_cursor: cursor,
            })
            .unwrap();
        gap |= batch.truncated;
        found_stderr |= batch
            .records
            .iter()
            .any(|record| record.stream == LogStream::Stderr && record.text.contains("日本"));
        cursor = batch.next_cursor;
        if batch.eof {
            break;
        }
    }
    assert!(gap && found_stderr);
    let directory = fixture
        .root
        .join("data/node-project-logs")
        .join(&flood.project_id);
    for name in ["events.jsonl", "events.1.jsonl"] {
        assert!(fs::metadata(directory.join(name)).unwrap().len() <= 5 * 1024 * 1024);
    }
    let restored = NodeProjects::new(fixture.state.layout.clone());
    assert_eq!(restored.runs().runs[0].state, RunState::Exited);
    assert!(
        restored
            .logs(LogRequest {
                id: flood.project_id,
                run_id: flood.run_id,
                after_cursor: 0
            })
            .unwrap()
            .truncated
    );
}

#[test]
#[ignore = "Requires the controlled runner example and two real runtimes"]
fn owned_forced_cleanup_corrupt_registry_and_failed_launch() {
    let fixture = Fixture::new();
    fixture.script(0, "stubborn");
    fixture.state.start(fixture.request(0)).unwrap();
    fixture.wait(0, |r| r.state == RunState::Running);
    let pids = fixture.pids(0);
    let before = fs::read(fixture.root.join("data/node-projects.json")).unwrap();
    fs::write(
        fixture.root.join("data/node-projects.json"),
        "corrupted registry",
    )
    .unwrap();
    assert!(fixture.state.snapshot().registry_error.is_some());
    let run = fixture.state.runs().runs[0].clone();
    let stopped = fixture
        .state
        .stop(StopRequest {
            id: run.project_id,
            run_id: run.run_id,
        })
        .unwrap();
    assert_eq!(stopped.state, RunState::Stopped);
    assert!(stopped.forced_cleanup);
    all_dead(&pids);
    fs::write(fixture.root.join("data/node-projects.json"), before).unwrap();
    let data = fixture.root.join("data");
    let (mut document, revision) = store::load(&data).unwrap();
    document.projects[0].runtime.version = "v99.0.0".into();
    store::save(&data, &revision, &document).unwrap();
    fixture.state.start(fixture.request(0)).unwrap();
    let failed = fixture.wait(0, |r| r.state == RunState::Failed);
    assert_eq!(failed.error.unwrap().code, ErrorCode::RuntimeChanged);
}

#[test]
#[ignore = "Requires the controlled runner example and two real runtimes"]
fn owned_app_crash_closes_liveness_channel_and_cleans_descendants() {
    let fixture = Fixture::new();
    fixture.script(0, "stubborn");
    let request = fixture.request(0);
    let mut owner = Command::new(&fixture.runner)
        .arg("--owner-fixture")
        .arg(&fixture.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = owner.stdin.take().unwrap();
    let mut output = BufReader::new(owner.stdout.take().unwrap());
    let mut call = |value: serde_json::Value| -> serde_json::Value {
        writeln!(input, "{value}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let response: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(response.get("error").is_none(), "{response}");
        response["value"].clone()
    };
    call(
        serde_json::json!({"command":"start","request":{"id":request.id,"expectedRevision":request.expected_revision,"manifestRevision":request.manifest_revision,"confirmed":true}}),
    );
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let state = call(serde_json::json!({"command":"runs"}));
        if state["runs"][0]["state"] == "running" {
            break;
        }
        assert!(Instant::now() < deadline, "{state}");
        thread::sleep(Duration::from_millis(20));
    }
    let pids = fixture.pids(0);
    owner.kill().unwrap();
    owner.wait().unwrap();
    drop(input);
    all_dead(&pids);
    let restored = NodeProjects::new(fixture.state.layout.clone());
    assert_eq!(restored.runs().runs[0].state, RunState::Failed);
}

#[test]
#[ignore = "Requires controlled runtimes, runner and FAIRWAY_TEST_RUNTIME packaged PHP tools"]
fn owned_lifecycle_quit_update_and_php_independence() {
    use crate::{
        config::Ports,
        lifecycle::{self, Lifecycle},
        manager::Manager,
    };
    use std::{
        io::Read,
        net::{TcpListener, TcpStream},
        sync::{Arc, Mutex},
    };
    let fixture = Fixture::new();
    assert!(fixture.state.runs().runs.is_empty()); // Import and app launch never start scripts.
    let mut layout = fixture.state.layout.clone().unwrap();
    layout.runtime = std::env::var_os("FAIRWAY_TEST_RUNTIME")
        .expect("Supply an existing read-only packaged runtime")
        .into();
    let registry = layout.data.join("node-projects.json");
    let original = fs::read(&registry).unwrap();
    fs::write(&registry, "invalid registry").unwrap();
    let mut manager = Manager::new(layout).unwrap();
    // If an assertion fails, stop the isolated PHP stack before Fixture deletes its directory.
    struct PhpGuard(Manager);
    impl Drop for PhpGuard {
        fn drop(&mut self) {
            let _ = self.0.stop_all();
        }
    }
    let listeners: Vec<_> = (0..6)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)).unwrap())
        .collect();
    let ports: Vec<_> = listeners
        .iter()
        .map(|s| s.local_addr().unwrap().port())
        .collect();
    drop(listeners);
    manager
        .save_ports(Ports {
            website: ports[0],
            phpmyadmin: ports[1],
            mysql: ports[2],
            php: ports[3],
            mail: ports[4],
            smtp: ports[5],
        })
        .unwrap();
    let mut php = PhpGuard(manager);
    fs::write(
        php.0.layout.www.join("node-independent.php"),
        "<?php echo 'php independent';",
    )
    .unwrap();
    php.0.start_all().unwrap(); // Broken Node configuration must not block PHP.
    fs::write(&registry, &original).unwrap();
    let assert_php = || {
        let mut stream = TcpStream::connect(("127.0.0.1", ports[0])).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"GET /node-independent.php HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.contains("php independent"), "{response}");
    };
    assert_php();
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = events.clone();
    fixture
        .state
        .observe_runs(move |run| observed.lock().unwrap().push(run));
    let first = fixture.state.start(fixture.request(0)).unwrap();
    fixture.wait(0, |s| s.state == RunState::Running);
    let pids = fixture.pids(0);
    php.0.stop_all().unwrap(); // PHP Stop All leaves the independently owned Node group alive.
    assert!(!dead(pids[0]));
    php.0.start_all().unwrap();
    assert_php();
    let lifecycle = Lifecycle::default();
    // The native hide handler never requests quit; no renderer owns the run.
    assert!(!lifecycle.quitting());
    let operation = lifecycle.begin().unwrap();
    let install = lifecycle.begin_install().unwrap();
    assert!(lifecycle.begin().is_err());
    assert!(!lifecycle.request_quit());
    fixture.state.stop_all_runs().unwrap(); // Same cleanup gate used before updater.install().
    all_dead(&pids);
    assert_php(); // Node Stop does not change PHP.
    drop(install); // Failed/cancelled installation releases its reservation.
    drop(operation);
    let states: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r.run_id == first.run_id)
        .map(|r| r.state)
        .collect();
    assert_eq!(states.first(), Some(&RunState::Starting));
    assert!(states.contains(&RunState::Running));
    assert!(states.contains(&RunState::Stopping));
    assert_eq!(states.last(), Some(&RunState::Stopped));
    // Stop is also valid immediately while runtime validation is still starting.
    let starting = fixture.state.start(fixture.request(1)).unwrap();
    assert_eq!(starting.state, RunState::Starting);
    assert_eq!(
        fixture
            .state
            .stop(StopRequest {
                id: starting.project_id,
                run_id: starting.run_id
            })
            .unwrap()
            .state,
        RunState::Stopped
    );
    fs::remove_file(Path::new(&fixture.projects[0].root).join("pids.json")).unwrap();
    fixture.state.start(fixture.request(0)).unwrap();
    fixture.wait(0, |s| s.state == RunState::Running);
    let pids = fixture.pids(0);
    assert!(lifecycle.request_quit());
    assert!(lifecycle.begin().is_err());
    lifecycle.wait_for_operation();
    lifecycle::stop_components(
        || fixture.state.stop_all_runs().map_err(|e| e.message),
        || php.0.stop_all(),
    )
    .unwrap();
    all_dead(&pids);
    lifecycle.allow_exit();
    assert!(lifecycle.exit_allowed());
    assert_eq!(fs::read(registry).unwrap(), original);
    fixture.state.disconnect_runs();
    assert_eq!(
        fixture.state.start(fixture.request(0)).unwrap_err().code,
        ErrorCode::ExecutionFailed
    );
}
