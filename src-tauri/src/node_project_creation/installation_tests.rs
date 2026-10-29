use super::*;
use crate::{
    config::Layout,
    node_project_creation::{
        self as creation, filesystem::Dir, CreationPlan, NodeProjectCreation, PlanRequest,
    },
    node_projects::{Inventory, NodeProjects, ProjectRuntime, RuntimeChoice},
};
use std::{fs, os::unix::fs::symlink, thread};
fn scratch() -> PathBuf {
    let root = std::env::temp_dir().join(format!("fw-npm-install-{:032x}", rand::random::<u128>()));
    fs::create_dir(&root).unwrap();
    fs::canonicalize(root).unwrap()
}
#[test]
fn genuine_locks_match_all_six_manifests_and_reject_missing_integrity_and_ranges() {
    for template in creation::catalog::catalog().templates {
        let lock: serde_json::Value =
            serde_json::from_slice(creation::locks::raw(&template.id).unwrap()).unwrap();
        let dependencies: BTreeMap<_, _> = template
            .dependencies
            .iter()
            .filter(|d| !d.development)
            .map(|d| (&d.name, &d.version))
            .collect();
        let dev: BTreeMap<_, _> = template
            .dependencies
            .iter()
            .filter(|d| d.development)
            .map(|d| (&d.name, &d.version))
            .collect();
        let package = serde_json::json!({"name":template.id,"version":"0.1.0","dependencies":dependencies,"devDependencies":dev});
        creation::locks::validate(&package, &lock).unwrap();
        let mut broken = lock.clone();
        broken["lockfileVersion"] = 2.into();
        assert!(creation::locks::validate(&package, &broken).is_err());
        let mut broken = lock.clone();
        broken["packages"][""]["dependencies"] = serde_json::json!({"unreviewed":"*"});
        assert!(creation::locks::validate(&package, &broken).is_err());
        if let Some(path) = lock["packages"]
            .as_object()
            .unwrap()
            .keys()
            .find(|path| !path.is_empty())
        {
            let mut broken = lock.clone();
            broken["packages"][path]["integrity"] = "sha512-forged".into();
            assert!(creation::locks::validate(&package, &broken).is_err());
            let mut broken = lock.clone();
            broken["packages"][path]["resolved"] = "http://untrusted/package.tgz".into();
            assert!(creation::locks::validate(&package, &broken).is_err());
        }
    }
}
#[test]
fn dependency_receipts_reject_escape_links_replacement_and_modification() {
    let root = scratch();
    fs::create_dir(root.join("node_modules")).unwrap();
    fs::create_dir(root.join("node_modules/.bin")).unwrap();
    fs::create_dir(root.join("node_modules/pkg")).unwrap();
    fs::write(root.join("node_modules/pkg/cli.js"), "reviewed").unwrap();
    symlink("../pkg/cli.js", root.join("node_modules/.bin/tool")).unwrap();
    let dir = Dir::absolute(&root).unwrap();
    let before = native::dependency_snapshot(&dir).unwrap();
    assert!(before.is_some());
    fs::write(root.join("node_modules/pkg/cli.js"), "changed").unwrap();
    assert_ne!(before, native::dependency_snapshot(&dir).unwrap());
    fs::remove_file(root.join("node_modules/.bin/tool")).unwrap();
    symlink("../../../personal", root.join("node_modules/.bin/tool")).unwrap();
    assert!(native::dependency_snapshot(&dir).is_err());
    fs::remove_file(root.join("node_modules/.bin/tool")).unwrap();
    fs::remove_dir_all(root.join("node_modules/pkg")).unwrap();
    symlink("/tmp", root.join("node_modules/pkg")).unwrap();
    symlink("../pkg/cli.js", root.join("node_modules/.bin/tool")).unwrap();
    assert!(native::dependency_snapshot(&dir).is_err());
    fs::remove_dir_all(root).unwrap();
}
fn inventory(layout: &Layout) -> Inventory {
    let node = layout.runtime.join("bin/node");
    Inventory {
        choices: vec![RuntimeChoice {
            id: "c".repeat(64),
            runtime: ProjectRuntime {
                node_path: node.to_str().unwrap().into(),
                version: fs::read_to_string(layout.runtime.join("version")).unwrap(),
            },
            source: "disposable fixture".into(),
        }],
        ..Default::default()
    }
}
struct Fixture {
    root: PathBuf,
    service: Arc<NodeProjectCreation>,
    nodes: Arc<NodeProjects>,
}
impl Fixture {
    fn new(node: &Path, fake: Option<&str>) -> Self {
        let root = scratch();
        for name in [
            "home",
            "projects 日本 with spaces",
            "tools/bin",
            "tools/lib/node_modules",
        ] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        let node = fs::canonicalize(node).unwrap();
        fs::copy(&node, root.join("tools/bin/node")).unwrap();
        let version = std::process::Command::new(&node)
            .env_clear()
            .args(["-p", "process.version"])
            .output()
            .unwrap();
        assert!(version.status.success());
        fs::write(
            root.join("tools/version"),
            String::from_utf8(version.stdout).unwrap().trim(),
        )
        .unwrap();
        let npm = fs::canonicalize(node.parent().unwrap().join("npm"))
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        if let Some(mode) = fake {
            let target = root.join("tools/lib/node_modules/npm");
            fs::create_dir_all(target.join("bin")).unwrap();
            fs::create_dir(target.join("node_modules")).unwrap();
            copy_tree(
                &npm.join("node_modules/semver"),
                &target.join("node_modules/semver"),
            );
            fs::write(target.join("package.json"),r#"{"name":"npm","version":"10.9.0","bin":{"npm":"bin/npm-cli.js"},"engines":{"node":">=22"}}"#).unwrap();
            let script = include_str!("installation_fixture.cjs").replace("__MODE__", mode);
            fs::write(target.join("bin/npm-cli.js"), script).unwrap();
        } else {
            symlink(&npm, root.join("tools/lib/node_modules/npm")).unwrap();
        }
        symlink(
            "../lib/node_modules/npm/bin/npm-cli.js",
            root.join("tools/bin/npm"),
        )
        .unwrap();
        fs::write(root.join("home/.npmrc"),"registry=http://127.0.0.1:1/\n//registry.npmjs.org/:_authToken=NEVER_INHERIT_FIXTURE\n").unwrap();
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("tools"));
        let mut service = NodeProjectCreation::new(Some(layout.clone()));
        service.discover = inventory;
        service.installation.executions.test_runner(
            std::env::var_os("FAIRWAY_TEST_NODE_RUNNER")
                .expect("Build fixture runner")
                .into(),
        );
        Self {
            root,
            service: Arc::new(service),
            nodes: Arc::new(NodeProjects::new(Some(layout))),
        }
    }
    fn node() -> PathBuf {
        std::env::var_os("FAIRWAY_TEST_NODE_RUNTIMES")
            .and_then(|paths| std::env::split_paths(&paths).next())
            .expect("Provide disposable Node runtime")
    }
    fn plan(&self, id: &str, name: &str) -> CreationPlan {
        self.service
            .plan(PlanRequest {
                request_id: format!("{:032x}", rand::random::<u128>()),
                catalog_revision: creation::catalog::revision(),
                template_id: id.into(),
                project_name: name.into(),
                parent: self
                    .root
                    .join("projects 日本 with spaces")
                    .to_str()
                    .unwrap()
                    .into(),
                runtime_id: "c".repeat(64),
                package_manager: "npm".into(),
            })
            .unwrap()
    }
    fn start(&self, plan: &CreationPlan) -> InstallStatus {
        self.service
            .start_installation(
                self.nodes.clone(),
                InstallRequest {
                    plan_id: plan.id.clone(),
                    digest: plan.digest.clone(),
                    confirmed: true,
                    install_confirmed: true,
                },
            )
            .unwrap()
    }
    fn wait(&self, job: &InstallStatus, terminal: bool) -> InstallStatus {
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let status = self
                .service
                .installation_status(InstallQuery {
                    job_id: job.job_id.clone(),
                })
                .unwrap();
            let active = self
                .service
                .installation
                .slots
                .lock()
                .unwrap()
                .get(&job.job_id)
                .unwrap()
                .active;
            if if terminal {
                !active
            } else {
                status.state == InstallState::Installing
            } {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "Installation did not settle: {status:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn cancel(&self, status: &InstallStatus) {
        loop {
            let status = self
                .service
                .installation_status(InstallQuery {
                    job_id: status.job_id.clone(),
                })
                .unwrap();
            match self.service.cancel_installation(InstallReference {
                job_id: status.job_id.clone(),
                expected_revision: status.revision,
            }) {
                Ok(_) => break,
                Err(error) if error.code == ErrorCode::StaleInstallation => (),
                Err(error) => panic!("{error:?}"),
            }
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.service.stop_installations();
        self.service.disconnect_installations();
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
#[ignore = "Requires controlled real runtime and runner"]
fn owned_installer_success_recovery_and_published_stage2_transition() {
    let f = Fixture::new(&Fixture::node(), Some("success"));
    let original = f.plan("node-js", "original");
    let job = f
        .service
        .generate(
            &f.nodes,
            creation::GenerateRequest {
                plan_id: original.id,
                digest: original.digest,
                confirmed: true,
            },
        )
        .unwrap();
    assert_eq!(job.phase, creation::generation::Phase::Completed);
    let before = fs::read(Path::new(&job.destination).join("package.json")).unwrap();
    let plan = f.plan("node-js", "installed-copy");
    let started = f.start(&plan);
    let done = f.wait(&started, true);
    assert_eq!(done.state, InstallState::Completed, "{done:?}");
    assert!(done.ci_completed);
    assert_eq!(f.nodes.snapshot().projects.len(), 2);
    assert!(f.nodes.runs().runs.is_empty());
    assert_eq!(
        before,
        fs::read(Path::new(&job.destination).join("package.json")).unwrap()
    );
    assert_eq!(
        f.service
            .start_installation(
                f.nodes.clone(),
                InstallRequest {
                    plan_id: plan.id.clone(),
                    digest: plan.digest.clone(),
                    confirmed: true,
                    install_confirmed: true
                }
            )
            .unwrap_err()
            .code,
        ErrorCode::PlanConsumed
    );
    assert_eq!(
        f.service
            .recover_installation(
                &f.nodes,
                InstallRecoveryRequest {
                    job_id: done.job_id,
                    expected_revision: done.revision,
                    action: InstallRecoveryAction::DiscardStaging,
                    confirmed: true
                }
            )
            .unwrap_err()
            .code,
        ErrorCode::StaleInstallation
    );
    // Registration conflict retains the fully installed published folder. Retry
    // refreshes registry authority and registers exactly once.
    let f = Fixture::new(&Fixture::node(), Some("slow-success"));
    let plan = f.plan("node-js", "retry-registration");
    let started = f.start(&plan);
    f.wait(&started, false);
    fs::write(
        f.root.join("data/node-projects.json"),
        "controlled corrupt registry",
    )
    .unwrap();
    let failed = f.wait(&started, true);
    assert_eq!(failed.state, InstallState::Failed, "{failed:?}");
    assert!(failed.ci_completed);
    assert!(Path::new(&plan.destination).exists());
    fs::remove_file(f.root.join("data/node-projects.json")).unwrap();
    let done = f
        .service
        .recover_installation(
            &f.nodes,
            InstallRecoveryRequest {
                job_id: failed.job_id.clone(),
                expected_revision: failed.revision,
                action: InstallRecoveryAction::Finalize,
                confirmed: true,
            },
        )
        .unwrap();
    assert_eq!(done.state, InstallState::Completed);
    assert_eq!(f.nodes.snapshot().projects.len(), 1);
    assert!(f
        .service
        .recover_installation(
            &f.nodes,
            InstallRecoveryRequest {
                job_id: failed.job_id,
                expected_revision: failed.revision,
                action: InstallRecoveryAction::Finalize,
                confirmed: true
            }
        )
        .is_err());
}
#[test]
#[ignore = "Requires controlled real runtime and runner"]
fn cancellation_stale_ids_duplicate_requests_runtime_lease_and_quit() {
    let f = Fixture::new(&Fixture::node(), Some("stubborn"));
    let plan = f.plan("node-js", "stubborn");
    let start = f.start(&plan);
    let running = f.wait(&start, false);
    assert!(f
        .service
        .ensure_install_runtime_unused(Path::new(&running.runtime.node_path))
        .is_err());
    assert_eq!(
        f.service
            .cancel_installation(InstallReference {
                job_id: running.job_id.clone(),
                expected_revision: 0
            })
            .unwrap_err()
            .code,
        ErrorCode::StaleInstallation
    );
    let second = f.plan("node-js", "second");
    assert_eq!(
        f.service
            .start_installation(
                f.nodes.clone(),
                InstallRequest {
                    plan_id: second.id,
                    digest: second.digest,
                    confirmed: true,
                    install_confirmed: true
                }
            )
            .unwrap_err()
            .code,
        ErrorCode::InstallBusy
    );
    f.cancel(&running);
    let end = f.wait(&start, true);
    assert_eq!(end.state, InstallState::Cancelled, "{end:?}");
    assert!(!Path::new(&plan.destination).exists());
    assert!(f
        .service
        .ensure_install_runtime_unused(Path::new(&end.runtime.node_path))
        .is_ok());
    assert!(f
        .service
        .installation_status(InstallQuery {
            job_id: "f".repeat(32)
        })
        .is_err());
    let pids: Vec<i32> = serde_json::from_slice(
        &fs::read(Path::new(&plan.parent).join(format!(
            ".fairway-node-{}/node_modules/fixture-pids.json",
            plan.id
        )))
        .unwrap(),
    )
    .unwrap();
    for pid in pids {
        assert!(dead(pid), "Leaked process {pid}");
    }
    let f = Fixture::new(&Fixture::node(), Some("stubborn"));
    let plan = f.plan("node-js", "quit");
    let start = f.start(&plan);
    f.wait(&start, false);
    f.service.cancel_installations_for_shutdown();
    f.service.stop_installations().unwrap();
    assert_eq!(f.wait(&start, true).state, InstallState::Cancelled);
}
#[test]
#[ignore = "Requires controlled real runtime and runner"]
fn failures_logs_source_races_and_interrupted_status_preserve_data() {
    for mode in ["fail", "flood", "replace-src", "touch-source"] {
        let f = Fixture::new(&Fixture::node(), Some(mode));
        let plan = f.plan("node-js", "fixture");
        let start = f.start(&plan);
        let done = f.wait(&start, true);
        assert_eq!(
            done.state,
            if mode == "flood" {
                InstallState::Completed
            } else {
                InstallState::Failed
            },
            "{mode}: {done:?}"
        );
        if mode == "fail" {
            assert_eq!(
                done.exit_code,
                Some(7),
                "Failed npm exit code must survive journal reload"
            );
        }
        assert!(f.nodes.runs().runs.is_empty());
        if mode != "flood" {
            assert!(!Path::new(&plan.destination).exists());
            assert!(f.nodes.snapshot().projects.is_empty());
        }
        if mode == "flood" {
            let logs = f
                .service
                .installation_logs(InstallLogRequest {
                    job_id: done.job_id.clone(),
                    after_cursor: 0,
                })
                .unwrap();
            assert!(logs.truncated);
            let mut cursor = logs.next_cursor;
            let mut text = logs
                .records
                .iter()
                .map(|r| r.text.clone())
                .collect::<String>();
            for _ in 0..30 {
                let logs = f
                    .service
                    .installation_logs(InstallLogRequest {
                        job_id: done.job_id.clone(),
                        after_cursor: cursor,
                    })
                    .unwrap();
                cursor = logs.next_cursor;
                text.extend(logs.records.iter().flat_map(|r| r.text.chars()));
                if logs.eof {
                    break;
                }
            }
            assert!(text.contains("日本"));
            assert!(!text.contains("NEVER_INHERIT_FIXTURE"));
        }
    }
    let f = Fixture::new(&Fixture::node(), Some("slow-success"));
    let plan = f.plan("node-js", "interrupted");
    let started = f.start(&plan);
    f.wait(&started, false);
    f.service.disconnect_installations();
    let done = f.wait(&started, true);
    assert_ne!(done.state, InstallState::Completed);
    let fresh = NodeProjectCreation::new(f.service.layout.clone());
    let status = fresh
        .installation_status(InstallQuery {
            job_id: started.job_id,
        })
        .unwrap();
    assert_ne!(status.state, InstallState::Completed);
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

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let dest = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &dest)
        } else {
            fs::copy(entry.path(), dest).unwrap();
        }
    }
}
#[cfg(feature = "node-install-fixtures")]
#[test]
#[ignore = "Requires explicit offline genuine tarball mirror, two real runtimes and runner"]
fn genuine_npm_ci_all_six_templates_node22_node24_and_registry_failures() {
    let nodes: Vec<_> =
        std::env::split_paths(&std::env::var_os("FAIRWAY_TEST_NODE_RUNTIMES").unwrap()).collect();
    assert_eq!(nodes.len(), 2);
    for node in &nodes {
        for template in creation::catalog::catalog().templates {
            let f = Fixture::new(node, None);
            *f.service.installation.fixture_registry.lock().unwrap() =
                Some(std::env::var("FAIRWAY_TEST_NPM_SUCCESS").unwrap());
            let plan = f.plan(&template.id, &format!("{}-qualified", template.id));
            let start = f.start(&plan);
            let done = f.wait(&start, true);
            if done.state != InstallState::Completed {
                let logs = f
                    .service
                    .installation_logs(InstallLogRequest {
                        job_id: done.job_id.clone(),
                        after_cursor: 0,
                    })
                    .unwrap();
                panic!(
                    "{} {}: {done:?} {logs:?}",
                    template.id, done.runtime.version
                );
            }
            assert!(done.ci_completed);
            let lock: serde_json::Value = serde_json::from_slice(
                &fs::read(Path::new(&plan.destination).join("package-lock.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(lock["name"], plan.project_name);
            assert!(f.nodes.runs().runs.is_empty());
            assert_eq!(f.nodes.snapshot().projects.len(), 1);
            // Existing Node controller executes only these reviewed disposable fixtures,
            // after explicit confirmation; installation itself starts no application.
            qualify_generated(&f, &plan);
        }
    }
    for mode in ["INTEGRITY", "ERROR"] {
        let f = Fixture::new(&nodes[0], None);
        *f.service.installation.fixture_registry.lock().unwrap() =
            Some(std::env::var(format!("FAIRWAY_TEST_NPM_{mode}")).unwrap());
        let plan = f.plan("express-js", "registry-failure");
        let start = f.start(&plan);
        let done = f.wait(&start, true);
        assert_eq!(done.state, InstallState::Failed, "{done:?}");
        assert!(!Path::new(&plan.destination).exists());
        assert!(f.nodes.snapshot().projects.is_empty());
        let logs = f
            .service
            .installation_logs(InstallLogRequest {
                job_id: done.job_id.clone(),
                after_cursor: 0,
            })
            .unwrap();
        let text = logs
            .records
            .iter()
            .map(|r| r.text.clone())
            .collect::<String>();
        assert!(
            text.contains(if mode == "INTEGRITY" {
                "EINTEGRITY"
            } else {
                "E503"
            }),
            "{text}"
        );
    }
}
#[cfg(feature = "node-install-fixtures")]
fn qualify_generated(f: &Fixture, plan: &CreationPlan) {
    use node_projects::{RunState, StartRequest, StopRequest};
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
    };
    let id = f.nodes.snapshot().projects[0].config.id.clone();
    let request = || StartRequest {
        id: id.clone(),
        expected_revision: f.nodes.snapshot().revision.unwrap(),
        manifest_revision: node_projects::inspect_generated_manifest(&plan.destination)
            .unwrap()
            .manifest_revision,
        confirmed: true,
    };
    // Select scripts by updating this fixture's existing registry atomically via
    // the public save contract, exactly as the ordinary Node workspace does.
    fn script(f: &Fixture, id: &str, script: &str) {
        let path = f.root.join("data/node-projects.json");
        let mut registry: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let project = registry["projects"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["id"] == id)
            .unwrap();
        project["script"] = script.into();
        fs::write(path, serde_json::to_vec_pretty(&registry).unwrap()).unwrap();
    }

    f.nodes
        .test_creation_runner(std::env::var_os("FAIRWAY_TEST_NODE_RUNNER").unwrap().into());
    let finite = if plan.template.scripts.contains_key("build") {
        Some("build")
    } else if plan.template.scripts.contains_key("test") {
        Some("test")
    } else {
        None
    };
    if let Some(finite) = finite {
        script(f, &id, finite);
        let run = f.nodes.start(request()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let status = f
                .nodes
                .runs()
                .runs
                .into_iter()
                .find(|r| r.run_id == run.run_id)
                .unwrap();
            if matches!(status.state, RunState::Exited | RunState::Failed) {
                assert_eq!(status.exit_code, Some(0), "{status:?}");
                break;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(25));
        }
        f.nodes
            .stop(StopRequest {
                id: id.clone(),
                run_id: run.run_id,
            })
            .unwrap();
    }
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    if plan.template.id.starts_with("react-vite") {
        let path = Path::new(&plan.destination).join("package.json");
        let mut package: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        package["scripts"]["dev"] = format!("vite --port {port} --strictPort").into();
        fs::write(path, serde_json::to_vec_pretty(&package).unwrap()).unwrap();
    } else {
        fs::write(
            Path::new(&plan.destination).join(".env"),
            format!("HOST=127.0.0.1\nPORT={port}\n"),
        )
        .unwrap();
    }
    script(f, &id, "dev");
    let run = f.nodes.start(request()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .write_all(b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(
                response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200"),
                "{response}"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "No dev server for {}",
            plan.template.id
        );
        thread::sleep(Duration::from_millis(25));
    }
    f.nodes
        .stop(StopRequest {
            id,
            run_id: run.run_id,
        })
        .unwrap();
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[test]
#[ignore = "Requires controlled real runtime and runner"]
fn abrupt_install_host_crash_kills_descendants_and_preserves_interrupted_workspace() {
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
    };
    let f = Fixture::new(&Fixture::node(), Some("stubborn"));
    let runner = std::env::var_os("FAIRWAY_TEST_NODE_RUNNER").unwrap();
    let mut owner = Command::new(runner)
        .arg("--install-owner-fixture")
        .arg(&f.root)
        .env(
            "PATH",
            std::env::join_paths([
                f.root.join("tools/bin"),
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
            ])
            .unwrap(),
        )
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = owner.stdin.take().unwrap();
    let mut output = BufReader::new(owner.stdout.take().unwrap());
    let mut call = |value: serde_json::Value| {
        writeln!(input, "{value}").unwrap();
        input.flush().unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let response: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert!(response.get("error").is_none(), "{response}");
        response["value"].clone()
    };
    let catalog = call(serde_json::json!({"command":"catalog"}));
    let runtime = catalog["runtimes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|runtime| runtime["nodePath"] == f.root.join("tools/bin/node").to_str().unwrap())
        .expect("Controlled runtime discovered");
    let plan:CreationPlan=serde_json::from_value(call(serde_json::json!({"command":"plan","request":{"requestId":"a".repeat(32),"catalogRevision":catalog["catalogRevision"],"templateId":"node-js","projectName":"crash-fixture","parent":f.root.join("projects 日本 with spaces"),"runtimeId":runtime["id"],"packageManager":"npm"}}))).unwrap();
    let started:InstallStatus=serde_json::from_value(call(serde_json::json!({"command":"start","request":{"planId":plan.id,"digest":plan.digest,"confirmed":true,"installConfirmed":true}}))).unwrap();
    let pids_path = Path::new(&plan.parent).join(format!(
        ".fairway-node-{}/node_modules/fixture-pids.json",
        plan.id
    ));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pids_path.exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    let pids: Vec<i32> = serde_json::from_slice(&fs::read(&pids_path).unwrap()).unwrap();
    owner.kill().unwrap();
    owner.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while !pids.iter().all(|pid| dead(*pid)) {
        assert!(
            Instant::now() < deadline,
            "Leaked fixture descendants: {pids:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    let status = f
        .service
        .installation_status(InstallQuery {
            job_id: started.job_id.clone(),
        })
        .unwrap();
    assert_eq!(status.state, InstallState::Interrupted);
    assert!(!Path::new(&plan.destination).exists());
    assert!(pids_path.exists());
    assert!(f.nodes.snapshot().projects.is_empty());
    assert!(f
        .service
        .recover_installation(
            &f.nodes,
            InstallRecoveryRequest {
                job_id: started.job_id,
                expected_revision: status.revision,
                action: InstallRecoveryAction::DiscardStaging,
                confirmed: true
            }
        )
        .is_err());
    assert!(pids_path.exists());
}
#[test]
#[ignore = "Requires controlled real runtime and runner"]
fn prelaunch_lock_runtime_and_consent_revalidation_and_owned_empty_cleanup() {
    for mutation in [
        "missing-lock",
        "invalid-lock",
        "missing-runtime",
        "modified-npm",
        "lock-held",
        "none",
    ] {
        let f = Fixture::new(&Fixture::node(), Some("success"));
        let plan = f.plan("node-js", "prelaunch");
        assert_eq!(
            f.service
                .start_installation(
                    f.nodes.clone(),
                    InstallRequest {
                        plan_id: plan.id.clone(),
                        digest: plan.digest.clone(),
                        confirmed: true,
                        install_confirmed: false
                    }
                )
                .unwrap_err()
                .code,
            ErrorCode::ConfirmationRequired
        );
        assert!(!f.root.join("data").exists());
        let status = InstallStatus {
            job_id: format!("{:032x}", rand::random::<u128>()),
            creation_id: plan.id.clone(),
            run_id: format!("{:032x}", rand::random::<u128>()),
            revision: 1,
            state: InstallState::Starting,
            runtime: ProjectRuntime {
                node_path: plan.tools.node_path.clone(),
                version: plan.tools.node_version.clone(),
            },
            lockfile_sha256: plan.files["package-lock.json"].sha256.clone(),
            registry: "https://registry.npmjs.org/".into(),
            ci_completed: false,
            cancel_requested: false,
            exit_code: None,
            error: None,
        };
        let job = f
            .service
            .stage_install(
                &f.nodes,
                creation::GenerateRequest {
                    plan_id: plan.id.clone(),
                    digest: plan.digest.clone(),
                    confirmed: true,
                },
                status.clone(),
            )
            .unwrap();
        assert_eq!(job.phase, creation::generation::Phase::Ready);
        let stage = Path::new(&job.staging_path);
        match mutation {
            "missing-lock" => fs::remove_file(stage.join("package-lock.json")).unwrap(),
            "invalid-lock" => fs::write(stage.join("package-lock.json"), "{}").unwrap(),
            "missing-runtime" => fs::remove_file(&plan.tools.node_path).unwrap(),
            "modified-npm" => {
                let path = &plan.tools.npm_cli_path;
                let mut bytes = fs::read(path).unwrap();
                bytes.extend_from_slice(b"\n// modified after consent\n");
                fs::write(path, bytes).unwrap();
            }
            "lock-held" => {
                let root = Dir::absolute(stage).unwrap();
                let _lease = root.lock_existing(".fairway-install.lock").unwrap();
                assert!(f
                    .service
                    .ensure_install_runtime_unused(Path::new(&plan.tools.node_path))
                    .is_err());
                assert!(f
                    .service
                    .recover_installation(
                        &f.nodes,
                        InstallRecoveryRequest {
                            job_id: status.job_id.clone(),
                            expected_revision: 1,
                            action: InstallRecoveryAction::DiscardStaging,
                            confirmed: true
                        }
                    )
                    .is_err());
                continue;
            }
            _ => (),
        }
        if mutation != "none" {
            assert!(native::runner_command(
                f.service.layout().unwrap(),
                &plan.id,
                &status.job_id,
                &status.run_id
            )
            .is_err());
            assert!(stage.exists());
            assert!(!Path::new(&plan.destination).exists());
        } else {
            let done = f
                .service
                .recover_installation(
                    &f.nodes,
                    InstallRecoveryRequest {
                        job_id: status.job_id,
                        expected_revision: 1,
                        action: InstallRecoveryAction::DiscardStaging,
                        confirmed: true,
                    },
                )
                .unwrap();
            assert_eq!(done.state, InstallState::Cancelled);
            assert!(!stage.exists());
        }
    }
    // A late foreign stage must remain untouched, and a rejected start must not
    // leave its discoverable installation status stuck in Starting forever.
    let f = Fixture::new(&Fixture::node(), Some("success"));
    let plan = f.plan("node-js", "occupied-stage");
    let stage = Path::new(&plan.parent).join(format!(".fairway-node-{}", plan.id));
    fs::create_dir(&stage).unwrap();
    fs::write(stage.join("foreign.txt"), "preserve me").unwrap();
    assert!(f
        .service
        .start_installation(
            f.nodes.clone(),
            InstallRequest {
                plan_id: plan.id.clone(),
                digest: plan.digest.clone(),
                confirmed: true,
                install_confirmed: true
            }
        )
        .is_err());
    let jobs = f.service.creation_jobs();
    assert_eq!(jobs.jobs.len(), 1);
    assert!(!jobs.jobs[0].active);
    assert_eq!(
        jobs.jobs[0].installation.as_ref().unwrap().state,
        InstallState::Failed
    );
    assert_eq!(
        fs::read_to_string(stage.join("foreign.txt")).unwrap(),
        "preserve me"
    );
    assert!(!Path::new(&plan.destination).exists());
}

#[test]
#[ignore = "Requires controlled real runtime and runner"]
fn npm_exit_waits_for_descendants_before_verification_publication_and_lease_release() {
    for action in ["complete", "cancel", "quit", "force"] {
        let f = Fixture::new(
            &Fixture::node(),
            Some(if action == "force" {
                "late-stubborn"
            } else {
                "late-child"
            }),
        );
        let plan = f.plan("node-js", "late-descendant");
        let started = f.start(&plan);
        let stage = Path::new(&plan.parent).join(format!(".fairway-node-{}", plan.id));
        let deadline = Instant::now() + Duration::from_secs(10);
        let run = loop {
            let status = f
                .service
                .installation
                .executions
                .statuses()
                .into_iter()
                .find(|run| run.run_id == started.run_id);
            if let Some(run) = status {
                if run.exit_code == Some(0) && run.state == node_projects::RunState::Stopping {
                    break run;
                }
            }
            assert!(
                Instant::now() < deadline,
                "Did not observe npm exit before child cleanup"
            );
            thread::sleep(Duration::from_millis(5));
        };
        let pids: Vec<i32> = serde_json::from_slice(
            &fs::read(stage.join("node_modules/fixture-pids.json")).unwrap(),
        )
        .unwrap();
        assert!(!dead(pids[1]));
        let store = creation::journal::Store::open(f.service.layout().unwrap(), false)
            .unwrap()
            .unwrap();
        let journal = store.load(&plan.id).unwrap();
        assert!(!journal.installation.unwrap().ci_completed);
        assert!(journal.dependencies.is_none());
        assert!(f
            .service
            .ensure_install_runtime_unused(Path::new(&run.runtime.node_path))
            .is_err());
        assert!(!Path::new(&plan.destination).exists());
        assert!(f.nodes.snapshot().projects.is_empty());
        if action == "cancel" {
            f.cancel(&started);
        } else if action == "quit" {
            f.service.cancel_installations_for_shutdown();
            f.service.stop_installations().unwrap();
        }
        let done = f.wait(&started, true);
        assert!(pids.iter().all(|pid| dead(*pid)));
        assert!(f
            .service
            .ensure_install_runtime_unused(Path::new(&done.runtime.node_path))
            .is_ok());
        match action {
            "complete" => {
                assert_eq!(done.state, InstallState::Completed, "{done:?}");
                assert_eq!(
                    fs::read_to_string(
                        Path::new(&plan.destination).join("node_modules/after-cleanup.txt")
                    )
                    .unwrap(),
                    "cleanup finished 日本"
                );
                assert!(done.ci_completed);
            }
            "force" => {
                assert_eq!(done.state, InstallState::Failed, "{done:?}");
                assert!(!done.ci_completed);
                assert!(!Path::new(&plan.destination).exists());
                assert!(f.nodes.snapshot().projects.is_empty());
            }
            _ => {
                assert_eq!(done.state, InstallState::Cancelled, "{done:?}");
                assert!(!done.ci_completed);
                assert!(!Path::new(&plan.destination).exists());
                assert!(f.nodes.snapshot().projects.is_empty());
            }
        }
    }
}

#[test]
#[ignore = "Requires controlled runtime/runner and read-only packaged PHP tools"]
fn installer_php_start_stop_and_updater_cleanup_are_independent() {
    use crate::{
        config::Ports,
        lifecycle::{self, Lifecycle},
        manager::Manager,
    };
    use std::{
        io::{Read, Write},
        net::{TcpListener, TcpStream},
    };
    let f = Fixture::new(&Fixture::node(), Some("stubborn"));
    let mut layout = f.service.layout().unwrap().clone();
    layout.runtime = std::env::var_os("FAIRWAY_TEST_RUNTIME")
        .expect("Supply read-only packaged PHP runtime")
        .into();
    struct Php(Manager);
    impl Drop for Php {
        fn drop(&mut self) {
            let _ = self.0.stop_all();
        }
    }
    let mut php = Php(Manager::new(layout).unwrap());
    let listeners: Vec<_> = (0..6)
        .map(|_| TcpListener::bind("127.0.0.1:0").unwrap())
        .collect();
    let ports: Vec<_> = listeners
        .iter()
        .map(|listener| listener.local_addr().unwrap().port())
        .collect();
    drop(listeners);
    php.0
        .save_ports(Ports {
            website: ports[0],
            phpmyadmin: ports[1],
            mysql: ports[2],
            php: ports[3],
            mail: ports[4],
            smtp: ports[5],
        })
        .unwrap();
    fs::write(
        php.0.layout.www.join("installer-independent.php"),
        "<?php echo 'PHP alive during npm installation';",
    )
    .unwrap();
    let plan = f.plan("node-js", "php-independent-installer");
    let start = f.start(&plan);
    f.wait(&start, false);
    let pids_path = Path::new(&plan.parent).join(format!(
        ".fairway-node-{}/node_modules/fixture-pids.json",
        plan.id
    ));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !pids_path.exists() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let pids: Vec<i32> = serde_json::from_slice(&fs::read(pids_path).unwrap()).unwrap();
    php.0.start_all().unwrap();
    let assert_php = || {
        let mut stream = TcpStream::connect(("127.0.0.1", ports[0])).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(b"GET /installer-independent.php HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.contains("PHP alive during npm installation"),
            "{response}"
        );
    };
    assert_php();
    php.0.stop_all().unwrap();
    assert!(pids.iter().all(|pid| !dead(*pid)));
    php.0.start_all().unwrap();
    assert_php();
    // Same guarded drain used before updater replacement. Hidden windows perform
    // no cancellation; updater/quit reserve operations before requesting cleanup.
    let lifecycle = Lifecycle::default();
    assert!(!lifecycle.quitting());
    let operation = lifecycle.begin().unwrap();
    let update = lifecycle.begin_install().unwrap();
    assert!(lifecycle.begin().is_err());
    f.service.cancel_installations_for_shutdown();
    f.service.stop_installations().unwrap();
    assert_eq!(f.wait(&start, true).state, InstallState::Cancelled);
    assert!(pids.iter().all(|pid| dead(*pid)));
    assert_php();
    drop(update);
    drop(operation);
    assert!(lifecycle.request_quit());
    lifecycle.wait_for_operation();
    lifecycle::stop_components(
        || {
            f.service
                .stop_installations()
                .map_err(|error| error.message)
        },
        || php.0.stop_all(),
    )
    .unwrap();
    assert!(!Path::new(&plan.destination).exists());
    assert!(f.nodes.runs().runs.is_empty());
}

#[cfg(feature = "node-install-fixtures")]
#[test]
#[ignore = "Requires explicit offline registry/network-error fixture, runtime and runner"]
fn genuine_npm_network_unavailable_preserves_stage_without_publication() {
    let f = Fixture::new(&Fixture::node(), None);
    *f.service.installation.fixture_registry.lock().unwrap() =
        Some(std::env::var("FAIRWAY_TEST_NPM_UNAVAILABLE").unwrap());
    let plan = f.plan("express-js", "network-unavailable");
    let started = f.start(&plan);
    let done = f.wait(&started, true);
    assert_eq!(done.state, InstallState::Failed, "{done:?}");
    assert!(!done.ci_completed);
    assert_eq!(done.exit_code, Some(1));
    assert!(!Path::new(&plan.destination).exists());
    assert!(f.nodes.snapshot().projects.is_empty());
    let logs = f
        .service
        .installation_logs(InstallLogRequest {
            job_id: done.job_id.clone(),
            after_cursor: 0,
        })
        .unwrap();
    assert!(
        logs.records
            .iter()
            .any(|record| record.text.contains("ECONNREFUSED")
                || record.text.contains("Exit handler never called!")),
        "{done:?} {logs:?}"
    );
    assert!(Path::new(&plan.parent)
        .join(format!(".fairway-node-{}", plan.id))
        .exists());
}
