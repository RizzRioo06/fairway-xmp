use super::*;
use std::{fs, path::PathBuf};
struct Fixture {
    root: PathBuf,
    state: NodeProjectCreation,
}
fn inventory(layout: &Layout) -> node_projects::Inventory {
    let node = layout.runtime.join("bin/node");
    if !node.is_file() {
        return node_projects::Inventory::default();
    }
    node_projects::Inventory {
        choices: vec![RuntimeChoice {
            id: "c".repeat(64),
            runtime: ProjectRuntime {
                node_path: node.to_str().unwrap().into(),
                version: fs::read_to_string(&node).unwrap(),
            },
            source: "fixture".into(),
        }],
        ..Default::default()
    }
}
fn prepare(
    layout: &Layout,
    runtime: ProjectRuntime,
    parent: &Path,
    destination: &Path,
    node_range: &str,
    npm_range: &str,
) -> node_projects::Result<RuntimePreparation> {
    assert!(parent.is_dir());
    assert!(!destination.exists());
    assert_eq!(node_range, ">=22.12.0 <23 || >=24.0.0 <25");
    assert_eq!(npm_range, ">=10.0.0 <12");
    if runtime.version == "v18.0.0" {
        return Err(node_projects::Error {
            code: node_projects::ErrorCode::RequirementConflict,
            message: "Unsupported fixture version".into(),
        });
    }
    let npm = fs::read_to_string(layout.runtime.join("npm")).map_err(|_| node_projects::Error {
        code: node_projects::ErrorCode::NpmUnavailable,
        message: "Missing fixture npm".into(),
    })?;
    Ok(RuntimePreparation {
        project_id: String::new(),
        registry_revision: String::new(),
        manifest_revision: String::new(),
        runtime,
        npm: node_projects::NpmRuntime {
            cli_path: layout.runtime.join("npm").to_str().unwrap().into(),
            version: npm.clone(),
        },
        runtime_revision: digest(npm.as_bytes()),
    })
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("fw-creation-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(root.join("home")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("tools"));
        fs::create_dir_all(layout.runtime.join("bin")).unwrap();
        fs::write(layout.runtime.join("bin/node"), "v22.23.4").unwrap();
        fs::write(layout.runtime.join("npm"), "10.9.0").unwrap();
        fs::create_dir_all(root.join("destination 日本 with spaces")).unwrap();
        let mut state = NodeProjectCreation::new(Some(layout));
        state.supported = true;
        state.discover = inventory;
        state.prepare = prepare;
        Self { root, state }
    }
    fn request(&self) -> PlanRequest {
        PlanRequest {
            request_id: "a".repeat(32),
            catalog_revision: catalog::revision(),
            template_id: "node-js".into(),
            project_name: "example-project".into(),
            parent: self
                .root
                .join("destination 日本 with spaces")
                .to_str()
                .unwrap()
                .into(),
            runtime_id: "c".repeat(64),
            package_manager: "npm".into(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn reference(plan: &CreationPlan) -> PlanReference {
    PlanReference {
        plan_id: plan.id.clone(),
        digest: plan.digest.clone(),
    }
}
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink()
        {
            result.insert(
                path.clone(),
                fs::read_link(&path)
                    .unwrap()
                    .to_string_lossy()
                    .as_bytes()
                    .to_vec(),
            );
        } else if fs::symlink_metadata(&path).unwrap().is_dir() {
            result.insert(path.clone(), Vec::new());
            result.extend(tree(&path));
        } else {
            result.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    result
}
#[test]
fn catalog_has_six_versioned_planning_only_designs_and_exact_dependencies() {
    let catalog = catalog::catalog();
    assert_eq!(catalog.schema_version, 1);
    assert_eq!(catalog.templates.len(), 6);
    let mut ids = std::collections::HashSet::new();
    for template in catalog.templates {
        assert!(ids.insert(template.id));
        assert_eq!(template.revision, "1");
        assert_eq!(template.qualification, "planningOnly");
        assert_eq!(template.lockfile_status, "notPrepared");
        assert_eq!(template.package_manager.name, "npm");
        assert_eq!(template.provenance.kind, "fairwayAuthoredDesign");
        assert!(template.provenance.upstream_revision.is_none());
        assert!(template.provenance.reference_url.starts_with("https://"));
        let mut files = std::collections::HashSet::new();
        for file in template.intended_files {
            assert!(files.insert(file.clone()));
            assert!(!Path::new(&file).is_absolute());
            assert!(!file.contains(".."));
        }
        for dependency in template.dependencies {
            assert!(dependency.version.split('.').count() == 3);
            assert!(dependency
                .version
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.'));
            assert!(dependency.metadata_url.ends_with(&dependency.version));
        }
    }
}
#[test]
fn plans_are_immutable_idempotent_and_do_not_write_or_register() {
    let f = Fixture::new();
    fs::write(f.root.join("home/.zshrc"), "touch PERSONAL_PROFILE_RAN").unwrap();
    let before = tree(&f.root);
    let plan = f.state.plan(f.request()).unwrap();
    assert!(plan
        .destination
        .ends_with("destination 日本 with spaces/example-project"));
    assert_eq!(plan.expires_at - plan.created_at, 600_000);
    assert_eq!(plan.template.id, "node-js");
    assert!(!plan.installation.installation_available);
    assert!(plan.installation.ignore_scripts);
    let again = f.state.plan(f.request()).unwrap();
    assert_eq!(again.id, plan.id);
    assert_eq!(again.digest, plan.digest);
    assert_eq!(
        f.state.validate_plan(reference(&plan)).unwrap().digest,
        plan.digest
    );
    assert_eq!(tree(&f.root), before);
    assert!(!f.root.join("data").exists());
    let mut edited = plan.clone();
    edited.destination = "/tmp/tampered".into();
    assert_eq!(
        f.state.validate_plan(reference(&plan)).unwrap().destination,
        plan.destination
    );
    let mut bad = reference(&edited);
    bad.digest = digest(&serde_json::to_vec(&edited).unwrap());
    assert_eq!(
        f.state.validate_plan(bad).unwrap_err().code,
        ErrorCode::PlanTampered
    );
}
#[test]
fn invalid_names_paths_templates_and_request_fields_fail_without_writes() {
    let f = Fixture::new();
    let before = tree(&f.root);
    for name in [
        "",
        ".",
        "..",
        "../escape",
        "UPPER",
        "has space",
        "日本",
        "-option",
        "trailing-",
        "con",
        "a/b",
        "x\\y",
        "x\0y",
    ] {
        let mut request = f.request();
        request.project_name = name.into();
        assert_eq!(
            f.state.plan(request).unwrap_err().code,
            ErrorCode::InvalidName,
            "{name}"
        );
    }
    let mut request = f.request();
    request.project_name = "x".repeat(65);
    assert_eq!(
        f.state.plan(request).unwrap_err().code,
        ErrorCode::InvalidName
    );
    for parent in [
        "relative",
        "/tmp/../escape",
        "/tmp/control\n",
        "/tmp/path:delimiter",
        "/definitely-missing-fairway-creation-parent",
    ] {
        let mut request = f.request();
        request.parent = parent.into();
        assert_eq!(
            f.state.plan(request).unwrap_err().code,
            ErrorCode::InvalidDestination,
            "{parent}"
        );
    }
    for (field, code) in [
        ("template", ErrorCode::InvalidTemplate),
        ("catalog", ErrorCode::CatalogChanged),
        ("pm", ErrorCode::UnsupportedPackageManager),
        ("runtime", ErrorCode::RuntimeUnavailable),
        ("request", ErrorCode::InvalidRequest),
    ] {
        let mut request = f.request();
        match field {
            "template" => request.template_id = "arbitrary-remote-url".into(),
            "catalog" => request.catalog_revision = "0".repeat(64),
            "pm" => request.package_manager = "yarn".into(),
            "runtime" => request.runtime_id = "d".repeat(64),
            _ => request.request_id = "bad".into(),
        }
        assert_eq!(f.state.plan(request).unwrap_err().code, code);
    }
    assert_eq!(tree(&f.root), before);
}
#[test]
fn existing_destinations_and_broken_links_are_rejected() {
    let f = Fixture::new();
    let request = f.request();
    let target = Path::new(&request.parent).join(&request.project_name);
    fs::create_dir(&target).unwrap();
    assert_eq!(
        f.state.plan(request.clone()).unwrap_err().code,
        ErrorCode::DestinationExists
    );
    fs::remove_dir(&target).unwrap();
    fs::write(&target, "keep").unwrap();
    assert_eq!(
        f.state.plan(request.clone()).unwrap_err().code,
        ErrorCode::DestinationExists
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "keep");
    fs::remove_file(&target).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(f.root.join("missing-target"), &target).unwrap();
        assert_eq!(
            f.state.plan(request).unwrap_err().code,
            ErrorCode::DestinationExists
        );
        assert!(fs::symlink_metadata(&target)
            .unwrap()
            .file_type()
            .is_symlink());
    }
}
#[test]
fn runtime_changes_missing_npm_and_unsupported_versions_are_safe() {
    let f = Fixture::new();
    let plan = f.state.plan(f.request()).unwrap();
    let tools = f.root.join("tools");
    fs::write(tools.join("npm"), "11.0.0").unwrap();
    assert_eq!(
        f.state.validate_plan(reference(&plan)).unwrap_err().code,
        ErrorCode::PlanStale
    );
    fs::remove_file(tools.join("npm")).unwrap();
    assert_eq!(
        f.state.plan(f.request()).unwrap_err().code,
        ErrorCode::NpmUnavailable
    );
    fs::write(tools.join("npm"), "10.9.0").unwrap();
    fs::write(tools.join("bin/node"), "v18.0.0").unwrap();
    let mut request = f.request();
    request.request_id = "b".repeat(32);
    assert_eq!(
        f.state.plan(request).unwrap_err().code,
        ErrorCode::RequirementConflict
    );
    fs::remove_file(tools.join("bin/node")).unwrap();
    assert_eq!(
        f.state.validate_plan(reference(&plan)).unwrap_err().code,
        ErrorCode::RuntimeUnavailable
    );
}
#[test]
fn expiration_duplicate_ids_and_restart_require_new_review() {
    let f = Fixture::new();
    let now = Instant::now();
    let plan = f.state.plan_at(f.request(), now, 1000).unwrap();
    assert_eq!(
        f.state
            .validate_at(reference(&plan), now + PLAN_TTL)
            .unwrap_err()
            .code,
        ErrorCode::PlanExpired
    );
    assert_eq!(
        f.state
            .plan_at(f.request(), now + PLAN_TTL, 601000)
            .unwrap_err()
            .code,
        ErrorCode::PlanExpired
    );
    let mut request = f.request();
    request.project_name = "different-project".into();
    assert_eq!(
        f.state.plan(request).unwrap_err().code,
        ErrorCode::DuplicateRequest
    );
    let other = Fixture::new();
    assert_eq!(
        other
            .state
            .validate_plan(reference(&plan))
            .unwrap_err()
            .code,
        ErrorCode::PlanUnavailable
    );
    let mut bad = reference(&plan);
    bad.digest = "f".repeat(64);
    assert_eq!(
        f.state.validate_plan(bad).unwrap_err().code,
        ErrorCode::PlanTampered
    );
    let invalid =
        serde_json::json!({"planId":plan.id,"digest":plan.digest,"destination":"/tmp/override"});
    assert!(serde_json::from_value::<PlanReference>(invalid).is_err());
}
#[test]
fn changing_parent_or_occupying_destination_invalidates_a_plan() {
    let f = Fixture::new();
    let request = f.request();
    let plan = f.state.plan(request.clone()).unwrap();
    fs::rename(&request.parent, f.root.join("old-parent")).unwrap();
    fs::create_dir(&request.parent).unwrap();
    assert_eq!(
        f.state.validate_plan(reference(&plan)).unwrap_err().code,
        ErrorCode::PlanStale
    );
    let mut request = f.request();
    request.request_id = "b".repeat(32);
    let plan = f.state.plan(request).unwrap();
    fs::create_dir(&plan.destination).unwrap();
    assert_eq!(
        f.state.validate_plan(reference(&plan)).unwrap_err().code,
        ErrorCode::DestinationExists
    );
}
#[test]
fn plan_capacity_is_bounded_and_expired_entries_are_reclaimed() {
    let f = Fixture::new();
    let now = Instant::now();
    for i in 0..PLAN_LIMIT {
        let mut request = f.request();
        request.request_id = format!("{i:032x}");
        f.state.plan_at(request, now, 1000).unwrap();
    }
    let mut request = f.request();
    request.request_id = "f".repeat(32);
    assert_eq!(
        f.state
            .plan_at(request.clone(), now, 1000)
            .unwrap_err()
            .code,
        ErrorCode::PlanLimit
    );
    f.state.plan_at(request, now + PLAN_TTL, 601000).unwrap();
    assert_eq!(f.state.plans.lock().unwrap().len(), 1);
}
#[test]
fn unsupported_platform_and_missing_layout_do_not_touch_files() {
    let f = Fixture::new();
    let before = tree(&f.root);
    let mut state = NodeProjectCreation::new(None);
    state.supported = false;
    assert!(!state.catalog().supported);
    assert_eq!(
        state.plan(f.request()).unwrap_err().code,
        ErrorCode::UnsupportedPlatform
    );
    state.supported = true;
    assert_eq!(
        state.plan(f.request()).unwrap_err().code,
        ErrorCode::Unavailable
    );
    assert_eq!(tree(&f.root), before);
}

#[cfg(unix)]
#[test]
fn unwritable_parents_and_parent_files_are_rejected_without_write_probes() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let parent = f.root.join("unwritable");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
    let mut request = f.request();
    request.parent = parent.to_str().unwrap().into();
    assert_eq!(
        f.state.plan(request).unwrap_err().code,
        ErrorCode::InvalidDestination
    );
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let file = f.root.join("parent-file");
    fs::write(&file, "keep").unwrap();
    let mut request = f.request();
    request.parent = file.to_str().unwrap().into();
    assert_eq!(
        f.state.plan(request).unwrap_err().code,
        ErrorCode::InvalidDestination
    );
    assert_eq!(fs::read_to_string(file).unwrap(), "keep");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "Supply two real Node/npm installations; runs alone because it owns process-local discovery variables"]
fn real_creation_planning_uses_inventory_and_paired_tools_without_writes_or_profiles() {
    use std::ffi::OsString;
    use std::process::Command;
    let nodes: Vec<_> = std::env::split_paths(
        &std::env::var_os("FAIRWAY_TEST_NODE_RUNTIMES").expect("Supply two installed runtimes"),
    )
    .map(|node| fs::canonicalize(node).unwrap())
    .collect();
    assert_eq!(nodes.len(), 2);
    let mut f = Fixture::new();
    let store = f.root.join("store");
    fs::create_dir_all(store.join("aliases")).unwrap();
    let mut versions = Vec::new();
    for node in &nodes {
        let output = Command::new(node)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-p", "process.version"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let version = String::from_utf8(output.stdout).unwrap().trim().to_owned();
        let directory = store.join("node-versions").join(&version);
        fs::create_dir_all(&directory).unwrap();
        std::os::unix::fs::symlink(
            node.parent().unwrap().parent().unwrap(),
            directory.join("installation"),
        )
        .unwrap();
        versions.push(version);
    }
    assert_ne!(versions[0], versions[1]);
    std::os::unix::fs::symlink(
        nodes[0].parent().unwrap().parent().unwrap(),
        store.join("aliases/default"),
    )
    .unwrap();
    let home = f.root.join("home");
    fs::write(home.join(".zshrc"), "/usr/bin/touch PROFILE_RAN\n").unwrap();
    fs::write(
        home.join(".npmrc"),
        "script-shell=/does-not-exist\nregistry=https://invalid.example/\n",
    )
    .unwrap();
    struct Restore(Vec<(&'static str, Option<OsString>)>);
    impl Drop for Restore {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
    let _restore = Restore(
        [
            "HOME",
            "ZDOTDIR",
            "FNM_DIR",
            "PATH",
            "NODE_OPTIONS",
            "NODE_PATH",
            "npm_config_registry",
        ]
        .into_iter()
        .map(|key| (key, std::env::var_os(key)))
        .collect(),
    );
    std::env::set_var("HOME", &home);
    std::env::set_var("ZDOTDIR", &home);
    std::env::set_var("FNM_DIR", &store);
    std::env::set_var("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    std::env::set_var("NODE_OPTIONS", "--invalid-inherited-option");
    std::env::set_var("NODE_PATH", f.root.join("poisoned"));
    std::env::set_var("npm_config_registry", "https://invalid.example/");
    f.state.discover = node_projects::discover_planning;
    f.state.prepare = node_projects::prepare_creation_runtime;
    let before = tree(&f.root);
    let snapshot = f.state.catalog();
    for (index, node) in nodes.iter().enumerate() {
        let choice = snapshot
            .runtimes
            .iter()
            .find(|choice| choice.runtime.node_path == node.to_str().unwrap())
            .expect("inventory discovers selected runtime");
        for (template_index, template) in snapshot.catalog.templates.iter().enumerate() {
            let mut request = f.request();
            request.request_id = format!("{:032x}", index * 10 + template_index);
            request.runtime_id = choice.id.clone();
            request.template_id = template.id.clone();
            let plan = f.state.plan(request).unwrap();
            assert_eq!(plan.tools.node_version, versions[index]);
            assert_eq!(plan.tools.node_path, node.to_str().unwrap());
            assert!(plan.tools.npm_cli_path.ends_with("npm/bin/npm-cli.js"));
            assert_eq!(
                f.state.validate_plan(reference(&plan)).unwrap().digest,
                plan.digest
            );
        }
        let runtime = choice.runtime.clone();
        let parent = Path::new(&f.request().parent).to_path_buf();
        let error = node_projects::prepare_creation_runtime(
            f.state.layout().unwrap(),
            runtime,
            &parent,
            &parent.join("not-created"),
            ">=99",
            ">=10 <12",
        )
        .unwrap_err();
        assert_eq!(error.code, node_projects::ErrorCode::RequirementConflict);
    }
    assert_eq!(tree(&f.root), before);
    assert!(!home.join("PROFILE_RAN").exists());
    assert!(!f.root.join("data").exists());
}

#[test]
fn concurrent_duplicate_submissions_return_one_native_plan() {
    let f = Fixture::new();
    let before = tree(&f.root);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let state = &f.state;
                let request = f.request();
                scope.spawn(move || state.plan(request).unwrap())
            })
            .collect();
        let plans: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        for plan in &plans {
            assert_eq!(plan.id, plans[0].id);
            assert_eq!(plan.digest, plans[0].digest);
        }
    });
    assert_eq!(f.state.plans.lock().unwrap().len(), 1);
    assert_eq!(tree(&f.root), before);
}

#[cfg(unix)]
#[test]
fn destination_links_to_existing_directories_and_files_are_never_adopted() {
    let f = Fixture::new();
    let request = f.request();
    let destination = Path::new(&request.parent).join(&request.project_name);
    let target = f.root.join("unrelated");
    fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, &destination).unwrap();
    assert_eq!(
        f.state.plan(request.clone()).unwrap_err().code,
        ErrorCode::DestinationExists
    );
    assert!(target.is_dir());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    fs::remove_file(&destination).unwrap();
    fs::remove_dir(&target).unwrap();
    fs::write(&target, "preserve unrelated file").unwrap();
    std::os::unix::fs::symlink(&target, &destination).unwrap();
    assert_eq!(
        f.state.plan(request).unwrap_err().code,
        ErrorCode::DestinationExists
    );
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        "preserve unrelated file"
    );
}

#[test]
fn ipc_shapes_are_camel_case_strict_and_exclude_environment_values() {
    let f = Fixture::new();
    let request = serde_json::to_value(f.request()).unwrap();
    assert_eq!(request.as_object().unwrap().len(), 7);
    assert!(request.get("requestId").is_some());
    assert!(request.get("runtimeId").is_some());
    let mut extra = request.clone();
    extra["nodePath"] = "/untrusted/bin/node".into();
    assert!(serde_json::from_value::<PlanRequest>(extra).is_err());
    let catalog = serde_json::to_value(f.state.catalog()).unwrap();
    assert_eq!(catalog["supported"], true);
    assert!(catalog["runtimes"][0].get("nodePath").is_some());
    let plan = serde_json::to_value(f.state.plan(f.request()).unwrap()).unwrap();
    assert_eq!(plan["installation"]["installationAvailable"], false);
    assert_eq!(plan["template"]["qualification"], "planningOnly");
    assert!(plan["tools"].get("revision").is_some());
    assert!(plan["tools"].get("environment").is_none());
    for code in [
        ErrorCode::PlanExpired,
        ErrorCode::PlanTampered,
        ErrorCode::CatalogChanged,
        ErrorCode::RuntimeChanged,
    ] {
        let value = serde_json::to_value(Error::new(code, "Review again.")).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 2);
        assert_eq!(value["message"], "Review again.");
        assert!(!value["code"].as_str().unwrap().contains('_'));
    }
}
