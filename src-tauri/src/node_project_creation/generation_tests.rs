use super::*;
use crate::node_project_creation::{filesystem::Dir, generation::Phase, journal::Store};
use crate::node_projects::NodeProjects;
use std::{
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicUsize, Ordering},
};
fn registry(f: &Fixture) -> NodeProjects {
    fs::set_permissions(
        f.root.join("tools/bin/node"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    NodeProjects::new(f.state.layout.clone())
}
fn generate_request(plan: &CreationPlan) -> GenerateRequest {
    GenerateRequest {
        plan_id: plan.id.clone(),
        digest: plan.digest.clone(),
        confirmed: true,
    }
}
fn recover_request(job: &JobStatus, action: RecoveryAction) -> RecoverRequest {
    RecoverRequest {
        job_id: job.job_id.clone(),
        expected_revision: job.revision,
        action,
        confirmed: true,
    }
}
fn fresh_service(f: &Fixture) -> NodeProjectCreation {
    let mut service = NodeProjectCreation::new(f.state.layout.clone());
    service.discover = inventory;
    service.prepare = prepare;
    service
}
fn injected() -> Error {
    Error::new(ErrorCode::FilesystemFailed, "Controlled fixture failure")
}
#[test]
fn every_template_generates_exact_reviewed_files_and_registers_without_execution() {
    let f = Fixture::new();
    let nodes = registry(&f);
    for (index, template) in catalog::catalog().templates.iter().enumerate() {
        let mut request = f.request();
        request.template_id = template.id.clone();
        request.project_name = format!("fixture-{index}");
        request.request_id = format!("{index:032x}");
        let plan = f.state.plan(request).unwrap();
        let blueprint = assets::render(&plan).unwrap();
        let job = f.state.generate(&nodes, generate_request(&plan)).unwrap();
        assert_eq!(job.phase, Phase::Completed, "{job:?}");
        assert!(!Path::new(&job.staging_path).exists());
        for (name, bytes) in blueprint {
            assert_eq!(
                fs::read(Path::new(&plan.destination).join(&name)).unwrap(),
                bytes,
                "{name}"
            );
        }
        let package: serde_json::Value = serde_json::from_slice(
            &fs::read(Path::new(&plan.destination).join("package.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            package["scripts"],
            serde_json::to_value(&template.scripts).unwrap()
        );
        assert_eq!(package["name"], plan.project_name);
        assert_eq!(package["packageManager"], "npm@10.9.0");
        assert_eq!(package["private"], true);
        assert!(!Path::new(&plan.destination).join("node_modules").exists());
        assert!(Path::new(&plan.destination)
            .join("package-lock.json")
            .exists());
        let snapshot = nodes.snapshot();
        let saved = snapshot
            .projects
            .iter()
            .find(|p| Some(&p.config.id) == job.project_id.as_ref())
            .unwrap();
        assert_eq!(saved.config.script, "dev");
        assert_eq!(saved.config.root, plan.destination);
        assert!(nodes.runs().runs.is_empty());
        assert_eq!(
            f.state
                .generate(&nodes, generate_request(&plan))
                .unwrap_err()
                .code,
            ErrorCode::PlanConsumed
        );
        assert_eq!(
            f.state
                .recover(
                    &nodes,
                    recover_request(&job, RecoveryAction::RetryRegistration)
                )
                .unwrap_err()
                .code,
            ErrorCode::StaleJob
        );
    }
    assert_eq!(nodes.snapshot().projects.len(), 6);
}
#[test]
fn expired_tampered_and_unconfirmed_plans_cannot_mutate() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let now = Instant::now();
    let expired = f
        .state
        .plan_at(f.request(), now - PLAN_TTL - Duration::from_secs(1), 1000);
    assert_eq!(expired.unwrap_err().code, ErrorCode::PlanExpired);
    let plan = f.state.plan(f.request()).unwrap();
    let before = tree(&f.root);
    let mut request = generate_request(&plan);
    request.confirmed = false;
    assert_eq!(
        f.state.generate(&nodes, request).unwrap_err().code,
        ErrorCode::ConfirmationRequired
    );
    let mut request = generate_request(&plan);
    request.digest = "f".repeat(64);
    assert_eq!(
        f.state.generate(&nodes, request).unwrap_err().code,
        ErrorCode::PlanTampered
    );
    // Simulate elapsed review time without waiting ten minutes.
    f.state
        .plans
        .lock()
        .unwrap()
        .values_mut()
        .for_each(|stored| stored.deadline = Instant::now() - Duration::from_secs(1));
    assert_eq!(
        f.state
            .generate(&nodes, generate_request(&plan))
            .unwrap_err()
            .code,
        ErrorCode::PlanExpired
    );
    assert_eq!(tree(&f.root), before);
}
#[test]
fn occupied_targets_and_replaced_parents_are_rejected_after_planning() {
    for link in [false, true] {
        let f = Fixture::new();
        let nodes = registry(&f);
        let plan = f.state.plan(f.request()).unwrap();
        if link {
            std::os::unix::fs::symlink(f.root.join("missing"), &plan.destination).unwrap();
        } else {
            fs::create_dir(&plan.destination).unwrap();
        }
        assert_eq!(
            f.state
                .generate(&nodes, generate_request(&plan))
                .unwrap_err()
                .code,
            ErrorCode::DestinationExists
        );
        assert!(!f.root.join("data").exists());
    }
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    fs::rename(&plan.parent, f.root.join("original-parent")).unwrap();
    fs::create_dir(&plan.parent).unwrap();
    assert_eq!(
        f.state
            .generate(&nodes, generate_request(&plan))
            .unwrap_err()
            .code,
        ErrorCode::PlanStale
    );
    assert_eq!(fs::read_dir(&plan.parent).unwrap().count(), 0);
}
#[test]
fn atomic_publication_does_not_replace_late_destinations() {
    for kind in [0, 1, 2] {
        let f = Fixture::new();
        let nodes = registry(&f);
        let plan = f.state.plan(f.request()).unwrap();
        let job = f
            .state
            .generate_inner(&nodes, generate_request(&plan), |point, journal| {
                if point == "atPublish" {
                    match kind {
                        0 => fs::create_dir(&journal.plan.destination).unwrap(),
                        1 => fs::write(&journal.plan.destination, "keep original").unwrap(),
                        _ => std::os::unix::fs::symlink(
                            f.root.join("missing"),
                            &journal.plan.destination,
                        )
                        .unwrap(),
                    }
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(job.phase, Phase::Preserved);
        assert!(Path::new(&job.staging_path).is_dir());
        assert!(nodes.snapshot().projects.is_empty());
        if kind == 1 {
            assert_eq!(
                fs::read_to_string(&plan.destination).unwrap(),
                "keep original"
            );
        }
        let discarded = f
            .state
            .recover(
                &nodes,
                recover_request(&job, RecoveryAction::DiscardStaging),
            )
            .unwrap();
        assert_eq!(discarded.phase, Phase::Cancelled);
        assert!(fs::symlink_metadata(&plan.destination).is_ok());
    }
    // Exercise the syscall's exclusive behavior directly, beyond preflight checks.
    let f = Fixture::new();
    let parent = Dir::absolute(Path::new(&f.request().parent)).unwrap();
    let _a = parent.make("source").unwrap();
    let _b = parent.make("target").unwrap();
    assert_eq!(
        parent.no_replace("source", "target").unwrap_err().code,
        ErrorCode::DestinationExists
    );
    assert!(parent.child("source").is_ok());
    assert!(parent.child("target").is_ok());
}
#[test]
fn cancellation_is_scoped_before_publication_and_cleans_only_receipted_data() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, journal| {
            if point == "file" {
                f.state.cancel_generation(CancelRequest {
                    job_id: journal.plan.id.clone(),
                })?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(job.phase, Phase::Cancelled, "{job:?}");
    assert!(!Path::new(&job.staging_path).exists());
    assert!(!Path::new(&plan.destination).exists());
    assert_eq!(
        f.state
            .cancel_generation(CancelRequest { job_id: job.job_id })
            .unwrap_err()
            .code,
        ErrorCode::JobUnavailable
    );
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, journal| {
            if point == "afterPublish" {
                assert_eq!(
                    f.state
                        .cancel_generation(CancelRequest {
                            job_id: journal.plan.id.clone()
                        })
                        .unwrap_err()
                        .code,
                    ErrorCode::CancellationTooLate
                );
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(job.phase, Phase::Completed);
}
#[test]
fn user_changes_unknown_entries_and_replaced_staging_are_preserved() {
    for kind in [0, 1, 2, 3] {
        let f = Fixture::new();
        let nodes = registry(&f);
        let plan = f.state.plan(f.request()).unwrap();
        let job = f
            .state
            .generate_inner(&nodes, generate_request(&plan), |point, journal| {
                if point == "ready" {
                    let stage = Path::new(&journal.plan.parent).join(&journal.stage_name);
                    match kind {
                        0 => fs::write(stage.join("README.md"), "my edits").unwrap(),
                        1 => fs::write(stage.join("unknown.txt"), "keep").unwrap(),
                        2 => {
                            fs::remove_file(stage.join("README.md")).unwrap();
                            std::os::unix::fs::symlink(
                                f.root.join("unrelated-file"),
                                stage.join("README.md"),
                            )
                            .unwrap();
                        }
                        _ => {
                            fs::rename(&stage, f.root.join("moved-owned-stage")).unwrap();
                            fs::create_dir(&stage).unwrap();
                            fs::write(stage.join("user.txt"), "keep").unwrap();
                        }
                    }
                    return Err(injected());
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(job.phase, Phase::Preserved);
        let before = tree(&f.root);
        let recovered = f
            .state
            .recover(
                &nodes,
                recover_request(&job, RecoveryAction::DiscardStaging),
            )
            .unwrap();
        assert_eq!(recovered.phase, Phase::Preserved);
        assert!(recovered.error.is_some());
        // Recovery updates only its journal, never uncertain source files.
        let mut after = tree(&f.root);
        let mut before = before;
        after.retain(|path, _| !path.starts_with(f.root.join("data")));
        before.retain(|path, _| !path.starts_with(f.root.join("data")));
        assert_eq!(after, before);
    }
}
#[test]
fn directory_replacement_and_ancestor_links_never_redirect_generation() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let replacement = f.root.join("unrelated");
    fs::create_dir(&replacement).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, journal| {
            if point == "staging" {
                let source = Path::new(&journal.plan.parent).join(&journal.stage_name);
                fs::rename(&source, f.root.join("moved-stage")).unwrap();
                std::os::unix::fs::symlink(&replacement, source).unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(job.phase, Phase::Preserved);
    assert_eq!(fs::read_dir(&replacement).unwrap().count(), 0);
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, journal| {
            if point == "beforePublish" {
                fs::rename(&journal.plan.parent, f.root.join("old-parent")).unwrap();
                fs::create_dir(&journal.plan.parent).unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(job.phase, Phase::Preserved);
    assert_eq!(fs::read_dir(&plan.parent).unwrap().count(), 0);
    assert!(nodes.snapshot().projects.is_empty());
    for unsafe_path in [
        "../escape",
        "/absolute",
        "src/../../escape",
        "src//x",
        "src/./x",
        "src\\evil",
        "x\0y",
    ] {
        assert_eq!(
            assets::validate_entry(unsafe_path).unwrap_err().code,
            ErrorCode::UnsafeTemplate
        );
    }
}
#[test]
fn failed_registration_retains_project_and_retry_is_revision_checked() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, _| {
            if point == "beforeRegistration" {
                fs::write(f.root.join("data/node-projects.json"), "corrupt registry").unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(job.phase, Phase::NeedsRegistration);
    assert!(Path::new(&plan.destination).is_dir());
    assert_eq!(
        fs::read_to_string(f.root.join("data/node-projects.json")).unwrap(),
        "corrupt registry"
    );
    let bytes = tree(Path::new(&plan.destination));
    let fresh = fresh_service(&f);
    let snapshot = fresh.creation_jobs();
    assert!(snapshot.jobs[0].recovery_required);
    let mut stale = recover_request(&job, RecoveryAction::RetryRegistration);
    stale.expected_revision -= 1;
    assert_eq!(
        fresh.recover(&nodes, stale).unwrap_err().code,
        ErrorCode::StaleJob
    );
    fs::write(
        f.root.join("data/node-projects.json"),
        r#"{"schemaVersion":1,"projects":[]}"#,
    )
    .unwrap();
    let recovered = fresh
        .recover(
            &nodes,
            recover_request(&job, RecoveryAction::RetryRegistration),
        )
        .unwrap();
    assert_eq!(recovered.phase, Phase::Completed, "{recovered:?}");
    assert_eq!(tree(Path::new(&plan.destination)), bytes);
    assert_eq!(nodes.snapshot().projects.len(), 1);
}
#[test]
fn post_publication_interruption_and_registry_commit_replay_are_idempotent() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, _| {
            if point == "afterPublish" {
                Err(injected())
            } else {
                Ok(())
            }
        })
        .unwrap();
    assert_eq!(job.phase, Phase::NeedsRegistration);
    let refused = f
        .state
        .recover(
            &nodes,
            recover_request(&job, RecoveryAction::DiscardStaging),
        )
        .unwrap();
    assert_eq!(refused.phase, Phase::Preserved);
    assert!(Path::new(&plan.destination).is_dir());
    let completed = f
        .state
        .recover(
            &nodes,
            recover_request(&refused, RecoveryAction::RetryRegistration),
        )
        .unwrap();
    assert_eq!(completed.phase, Phase::Completed);
    // Emulate a crash after registry publication but before the terminal journal save.
    let store = Store::open(f.state.layout().unwrap(), false)
        .unwrap()
        .unwrap();
    let mut journal = store.load(&completed.job_id).unwrap();
    journal.phase = Phase::NeedsRegistration;
    journal.project_id = None;
    store.save(&mut journal).unwrap();
    let old = fs::read(f.root.join("data/node-projects.json")).unwrap();
    let snapshot = f.state.creation_jobs();
    let recovered = f
        .state
        .recover(
            &nodes,
            recover_request(&snapshot.jobs[0], RecoveryAction::RetryRegistration),
        )
        .unwrap();
    assert_eq!(recovered.phase, Phase::Completed);
    assert_eq!(nodes.snapshot().projects.len(), 1);
    assert_eq!(
        fs::read(f.root.join("data/node-projects.json")).unwrap(),
        old
    );
}
#[test]
fn partial_file_journal_corruption_and_io_failures_preserve_uncertain_data() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, journal| {
            if point == "staging" {
                fs::write(
                    Path::new(&journal.plan.parent)
                        .join(&journal.stage_name)
                        .join("partial.tmp"),
                    "incomplete",
                )
                .unwrap();
                return Err(injected());
            }
            Ok(())
        })
        .unwrap();
    let recovered = f
        .state
        .recover(
            &nodes,
            recover_request(&job, RecoveryAction::DiscardStaging),
        )
        .unwrap();
    assert_eq!(recovered.phase, Phase::Preserved);
    assert_eq!(
        fs::read_to_string(Path::new(&job.staging_path).join("partial.tmp")).unwrap(),
        "incomplete"
    );
    fs::write(
        f.root
            .join("data/node-project-creation")
            .join(format!("{}.json", job.job_id)),
        "invalid journal",
    )
    .unwrap();
    let snapshot = fresh_service(&f).creation_jobs();
    assert!(!snapshot.errors.is_empty());
    assert!(snapshot.jobs.is_empty());
    assert!(Path::new(&job.staging_path).is_dir());
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let parent = Path::new(&plan.parent);
    fs::set_permissions(parent, fs::Permissions::from_mode(0o500)).unwrap();
    assert_eq!(
        f.state
            .generate(&nodes, generate_request(&plan))
            .unwrap_err()
            .code,
        ErrorCode::InvalidDestination
    );
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!f.root.join("data").exists());
}
#[test]
fn interrupted_generation_can_be_explicitly_discarded_after_restart() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let calls = AtomicUsize::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = f
            .state
            .generate_inner(&nodes, generate_request(&plan), |point, _| {
                if point == "file" && calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("simulated host interruption");
                }
                Ok(())
            });
    }));
    assert!(result.is_err());
    let restarted = fresh_service(&f);
    let snapshot = restarted.creation_jobs();
    assert_eq!(snapshot.jobs.len(), 1);
    assert!(snapshot.jobs[0].recovery_required);
    assert!(nodes.snapshot().projects.is_empty());
    let recovered = restarted
        .recover(
            &nodes,
            recover_request(&snapshot.jobs[0], RecoveryAction::DiscardStaging),
        )
        .unwrap();
    assert_eq!(recovered.phase, Phase::Cancelled);
    assert!(!Path::new(&recovered.staging_path).exists());
}

#[test]
fn abrupt_host_kill_leaves_durable_staging_and_publication_recoverable() {
    use std::{
        io::{BufRead, BufReader},
        process::{Command, Stdio},
        sync::mpsc,
    };
    for point in ["file", "afterPublish"] {
        let f = Fixture::new();
        let nodes = registry(&f);
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "node_project_creation::tests::generation_tests::generation_crash_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("FAIRWAY_GENERATION_CRASH_ROOT", &f.root)
                .env("FAIRWAY_GENERATION_CRASH_POINT", point)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let stdout = child.0.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if line
                    .unwrap_or_default()
                    .contains("GENERATION_FIXTURE_READY")
                {
                    let _ = send.send(());
                    break;
                }
            }
        });
        receive
            .recv_timeout(Duration::from_secs(15))
            .expect("controlled child reached journal boundary");
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        reader.join().unwrap();
        let service = fresh_service(&f);
        let snapshot = service.creation_jobs();
        assert_eq!(snapshot.jobs.len(), 1);
        assert!(snapshot.jobs[0].recovery_required);
        assert!(nodes.snapshot().projects.is_empty());
        let action = if point == "file" {
            RecoveryAction::DiscardStaging
        } else {
            RecoveryAction::RetryRegistration
        };
        let job = service
            .recover(&nodes, recover_request(&snapshot.jobs[0], action))
            .unwrap();
        assert_eq!(
            job.phase,
            if point == "file" {
                Phase::Cancelled
            } else {
                Phase::Completed
            },
            "{job:?}"
        );
        assert!(!Path::new(&job.staging_path).exists());
    }
}
#[test]
#[ignore = "Internal child transport used only by abrupt_host_kill; not an independent compatibility test"]
fn generation_crash_child() {
    use std::io::Write;
    let Some(root) = std::env::var_os("FAIRWAY_GENERATION_CRASH_ROOT") else {
        return;
    };
    let root = fs::canonicalize(root).unwrap();
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("fw-creation-"));
    let layout = Layout::at(root.join("home"), root.join("data"), root.join("tools"));
    let mut state = NodeProjectCreation::new(Some(layout));
    state.discover = inventory;
    state.prepare = prepare;
    let f = Fixture { root, state };
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let point = std::env::var("FAIRWAY_GENERATION_CRASH_POINT").unwrap();
    let _ = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |current, _| {
            if current == point {
                println!("GENERATION_FIXTURE_READY");
                std::io::stdout().flush().unwrap();
                std::thread::sleep(Duration::from_secs(30));
                return Err(injected());
            }
            Ok(())
        });
}

#[test]
fn nested_directory_replacement_never_receives_generated_writes() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let changed = std::sync::atomic::AtomicBool::new(false);
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, journal| {
            if point == "file" && !changed.swap(true, Ordering::SeqCst) {
                let path = Path::new(&journal.plan.parent)
                    .join(&journal.stage_name)
                    .join("src");
                fs::rename(&path, f.root.join("original-src")).unwrap();
                fs::create_dir(&path).unwrap();
                fs::write(path.join("user.txt"), "keep").unwrap();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(job.phase, Phase::Preserved);
    let path = Path::new(&job.staging_path).join("src");
    let names: Vec<_> = fs::read_dir(&path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(names, vec![std::ffi::OsString::from("user.txt")]);
    assert_eq!(fs::read_to_string(path.join("user.txt")).unwrap(), "keep");
}

#[test]
#[ignore = "Supply real installed Node/npm tools; metadata validation only, no generated scripts/installers are run"]
fn real_local_generation_uses_selected_runtime_without_installation() {
    fn real_inventory(layout: &Layout) -> node_projects::Inventory {
        let runtimes: Vec<node_projects::ProjectRuntime> =
            serde_json::from_slice(&fs::read(layout.runtime.join("real-runtime.json")).unwrap())
                .unwrap();
        node_projects::Inventory {
            choices: runtimes
                .into_iter()
                .map(|runtime| RuntimeChoice {
                    id: digest(&serde_json::to_vec(&runtime).unwrap()),
                    runtime,
                    source: "controlled real tools".into(),
                })
                .collect(),
            ..Default::default()
        }
    }
    let paths: Vec<_> = std::env::split_paths(
        &std::env::var_os("FAIRWAY_TEST_NODE_RUNTIMES").expect("Supply two real runtimes"),
    )
    .collect();
    assert_eq!(paths.len(), 2);
    let mut f = Fixture::new();
    let mut runtimes = Vec::new();
    for path in paths {
        let path = fs::canonicalize(path).unwrap();
        let output = std::process::Command::new(&path)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-p", "process.version"])
            .output()
            .unwrap();
        assert!(output.status.success());
        runtimes.push(node_projects::ProjectRuntime {
            node_path: path.to_str().unwrap().into(),
            version: String::from_utf8(output.stdout).unwrap().trim().into(),
        });
    }
    assert_ne!(runtimes[0].version, runtimes[1].version);
    fs::write(
        f.root.join("tools/real-runtime.json"),
        serde_json::to_vec(&runtimes).unwrap(),
    )
    .unwrap();
    f.state.discover = real_inventory;
    f.state.prepare = node_projects::prepare_creation_runtime;
    let nodes = NodeProjects::new(f.state.layout.clone());
    for (index, template) in catalog::catalog().templates.iter().enumerate() {
        let mut request = f.request();
        request.template_id = template.id.clone();
        request.request_id = format!("{index:032x}");
        request.project_name = format!("real-local-{index}");
        request.runtime_id = digest(&serde_json::to_vec(&runtimes[index % 2]).unwrap());
        let plan = f.state.plan(request).unwrap();
        let job = f.state.generate(&nodes, generate_request(&plan)).unwrap();
        assert_eq!(job.phase, Phase::Completed, "{job:?}");
        let root = Path::new(&plan.destination);
        assert!(!root.join("node_modules").exists());
        assert!(!root.join("package-lock.json").exists());
        assert_eq!(
            fs::read_to_string(root.join(".node-version"))
                .unwrap()
                .trim(),
            runtimes[index % 2].version
        );
    }
    assert_eq!(nodes.snapshot().projects.len(), 6);
    assert!(nodes.runs().runs.is_empty());
}

#[test]
fn missing_ownership_marker_and_incomplete_published_trees_are_not_adopted() {
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, _| {
            if point == "staging" {
                Err(injected())
            } else {
                Ok(())
            }
        })
        .unwrap();
    let stage = Path::new(&job.staging_path);
    fs::remove_file(stage.join(".fairway-template.json")).unwrap();
    let recovered = f
        .state
        .recover(
            &nodes,
            recover_request(&job, RecoveryAction::DiscardStaging),
        )
        .unwrap();
    assert_eq!(recovered.phase, Phase::Preserved);
    assert!(stage.is_dir());
    let f = Fixture::new();
    let nodes = registry(&f);
    let plan = f.state.plan(f.request()).unwrap();
    let job = f
        .state
        .generate_inner(&nodes, generate_request(&plan), |point, journal| {
            if point == "file" && journal.files.contains_key("package.json") {
                fs::rename(
                    Path::new(&journal.plan.parent).join(&journal.stage_name),
                    &journal.plan.destination,
                )
                .unwrap();
                return Err(injected());
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(job.phase, Phase::NeedsRegistration);
    assert!(Path::new(&job.destination).is_dir());
    let recovered = f
        .state
        .recover(
            &nodes,
            recover_request(&job, RecoveryAction::RetryRegistration),
        )
        .unwrap();
    assert_eq!(recovered.phase, Phase::NeedsRegistration);
    assert!(recovered.error.is_some());
    assert!(nodes.snapshot().projects.is_empty());
}
