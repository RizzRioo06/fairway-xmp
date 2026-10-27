use super::*;
use crate::node_projects::{
    runtime, InspectRequest, NodeProjects, PrepareRuntimeRequest, SaveConfig, SaveRequest,
};
use std::{ffi::OsString, sync::Mutex};

struct Fixture {
    base: PathBuf,
    layout: Layout,
    project: ProjectConfig,
}
impl Fixture {
    fn new() -> Self {
        let base =
            std::env::temp_dir().join(format!("fw-node-isolation-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let layout = Layout::at(base.join("home"), base.join("data"), base.join("tools"));
        fs::create_dir_all(&layout.home).unwrap();
        let prefix = layout.runtime.join("v22");
        fs::create_dir_all(prefix.join("bin")).unwrap();
        let node = prefix.join("bin/node");
        fs::write(&node, "FAKE RUNTIME: must not be executed").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&node, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let root = base.join("project 日本");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            br#"{"scripts":{"dev":"touch IMPORTED_SCRIPT_RAN"},"engines":{"node":">=20"}}"#,
        )
        .unwrap();
        let npm_root = prefix.join("lib/node_modules/npm");
        fs::create_dir_all(npm_root.join("bin")).unwrap();
        fs::write(npm_root.join("package.json"), br#"{"name":"npm","version":"10.9.0","bin":{"npm":"bin/npm-cli.js"},"engines":{"node":">=18"}}"#).unwrap();
        fs::write(
            npm_root.join("bin/npm-cli.js"),
            "FAKE NPM: must not be executed",
        )
        .unwrap();
        let project = ProjectConfig {
            id: "a".repeat(32),
            name: "Test".into(),
            root: root.to_str().unwrap().into(),
            script: "dev".into(),
            runtime: ProjectRuntime {
                node_path: node.to_str().unwrap().into(),
                version: "v22.1.0".into(),
            },
            open_url: None,
        };
        Self {
            base,
            layout,
            project,
        }
    }
    fn npm(&self) -> PathBuf {
        self.layout.runtime.join("v22/lib/node_modules/npm")
    }
    fn inspect(&self) -> Inspection {
        manifest::inspect(&self.project.root).unwrap()
    }
    fn prepare(&self) -> Result<PreparedRuntime> {
        prepare_with(
            &self.layout,
            &self.project,
            &self.inspect(),
            "registry-revision".into(),
            Vec::new(),
            fake_probe,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn fake_probe(command: Command) -> Result<String> {
    let args: Vec<_> = command.get_args().collect();
    assert!(command
        .get_program()
        .to_string_lossy()
        .ends_with("/bin/node"));
    assert_ne!(
        command.get_current_dir().unwrap().file_name().unwrap(),
        "project 日本"
    );
    if args.first().is_some_and(|arg| *arg == "-e") {
        let input: serde_json::Value =
            serde_json::from_str(args.last().unwrap().to_str().unwrap()).unwrap();
        Ok(serde_json::json!({"nodePath":input["nodePath"], "version":input["version"], "npmVersion":input["npmVersion"]}).to_string())
    } else {
        assert_eq!(args.last().unwrap().to_str(), Some("--version"));
        Ok("10.9.0".into())
    }
}

#[test]
fn npm_pairing_uses_the_selected_prefix_and_never_path_fallback() {
    let fixture = Fixture::new();
    let plan = fixture.prepare().unwrap();
    assert_eq!(plan.summary().runtime, fixture.project.runtime);
    assert!(plan
        .summary()
        .npm
        .cli_path
        .ends_with("v22/lib/node_modules/npm/bin/npm-cli.js"));
    fs::remove_dir_all(fixture.npm()).unwrap();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::NpmUnavailable
    );
}

#[cfg(unix)]
#[test]
fn explicit_npm_links_are_validated_and_broken_links_do_not_fall_back() {
    let fixture = Fixture::new();
    let anchor = fixture.layout.runtime.join("v22/bin/npm");
    std::os::unix::fs::symlink(fixture.npm().join("bin/npm-cli.js"), &anchor).unwrap();
    fixture.prepare().unwrap();
    fs::remove_file(&anchor).unwrap();
    std::os::unix::fs::symlink("missing/npm-cli.js", &anchor).unwrap();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::NpmUnavailable
    );
}

#[test]
fn ambiguous_npm_layouts_and_malformed_packages_are_rejected() {
    let fixture = Fixture::new();
    let other = fixture
        .layout
        .runtime
        .join("v22/libexec/lib/node_modules/npm");
    fs::create_dir_all(&other).unwrap();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::NpmUnavailable
    );
    fs::remove_dir_all(other).unwrap();
    for package in [
        "{",
        r#"{"name":"other"}"#,
        r#"{"name":"npm","version":"10.9.0","bin":{"npm":"../../project/script.js"},"engines":{"node":"*"}}"#,
    ] {
        fs::write(fixture.npm().join("package.json"), package).unwrap();
        assert_eq!(
            fixture.prepare().err().unwrap().code,
            ErrorCode::NpmUnavailable
        );
    }
}

#[test]
fn missing_cli_and_removed_or_non_executable_node_fail_before_any_probe() {
    let fixture = Fixture::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            &fixture.project.runtime.node_path,
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert_eq!(
            fixture.prepare().err().unwrap().code,
            ErrorCode::RuntimeUnavailable
        );
        fs::set_permissions(
            &fixture.project.runtime.node_path,
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
    }
    fs::remove_file(fixture.npm().join("bin/npm-cli.js")).unwrap();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::NpmUnavailable
    );
    fs::remove_file(&fixture.project.runtime.node_path).unwrap();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::RuntimeUnavailable
    );
}

#[cfg(unix)]
#[test]
fn changed_runtime_symlinks_are_not_silently_adopted() {
    let fixture = Fixture::new();
    let node = Path::new(&fixture.project.runtime.node_path);
    fs::rename(node, node.with_file_name("different-node")).unwrap();
    std::os::unix::fs::symlink("different-node", node).unwrap();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::RuntimeChanged
    );
}

#[test]
fn different_probe_version_and_wrong_npm_version_are_rejected() {
    let fixture = Fixture::new();
    let error = prepare_with(
        &fixture.layout,
        &fixture.project,
        &fixture.inspect(),
        "r".into(),
        Vec::new(),
        |command| {
            let output = fake_probe(command)?;
            Ok(output.replace("v22.1.0", "v24.0.0"))
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::RuntimeChanged);
    let error = prepare_with(
        &fixture.layout,
        &fixture.project,
        &fixture.inspect(),
        "r".into(),
        Vec::new(),
        |command| {
            if command
                .get_args()
                .last()
                .is_some_and(|arg| arg == "--version")
            {
                Ok("99.0.0".into())
            } else {
                fake_probe(command)
            }
        },
    )
    .err()
    .unwrap();
    assert_eq!(error.code, ErrorCode::NpmUnavailable);
}

#[test]
fn modifications_during_preparation_are_detected() {
    for change_node in [true, false] {
        let fixture = Fixture::new();
        let mut calls = 0;
        let error = prepare_with(
            &fixture.layout,
            &fixture.project,
            &fixture.inspect(),
            "r".into(),
            Vec::new(),
            |command| {
                calls += 1;
                if calls == 1 {
                    fs::write(
                        if change_node {
                            PathBuf::from(&fixture.project.runtime.node_path)
                        } else {
                            fixture.npm().join("bin/npm-cli.js")
                        },
                        "changed bytes",
                    )
                    .unwrap();
                }
                fake_probe(command)
            },
        )
        .err()
        .unwrap();
        assert_eq!(
            error.code,
            if change_node {
                ErrorCode::RuntimeChanged
            } else {
                ErrorCode::NpmUnavailable
            }
        );
    }
}

#[test]
fn child_environment_drops_inherited_runtime_controls_and_secrets() {
    let fixture = Fixture::new();
    let names = [
        "PATH",
        "Path",
        "NODE_OPTIONS",
        "NODE_PATH",
        "NODE",
        "FNM_DIR",
        "FNM_MULTISHELL_PATH",
        "NVM_BIN",
        "NVM_DIR",
        "VOLTA_HOME",
        "BASH_ENV",
        "ENV",
        "ZDOTDIR",
        "DYLD_INSERT_LIBRARIES",
        "LD_PRELOAD",
        "npm_execpath",
        "npm_node_execpath",
        "npm_config_script_shell",
        "NPM_CONFIG_PREFIX",
        "npm_config_node_options",
        "SECRET_TOKEN",
    ];
    let mut inherited: Vec<_> = names
        .iter()
        .map(|name| (OsString::from(name), OsString::from("poison")))
        .collect();
    inherited.push(("LANG".into(), "en_US.UTF-8".into()));
    let plan = prepare_with(
        &fixture.layout,
        &fixture.project,
        &fixture.inspect(),
        "r".into(),
        inherited.clone(),
        fake_probe,
    )
    .unwrap();
    let env = plan.environment();
    for name in names {
        assert_ne!(
            env.get(std::ffi::OsStr::new(name)),
            Some(&OsString::from("poison")),
            "{name}"
        );
    }
    assert_eq!(
        env.get(std::ffi::OsStr::new("LANG")),
        Some(&OsString::from("en_US.UTF-8"))
    );
    let paths: Vec<_> = std::env::split_paths(&env[std::ffi::OsStr::new("PATH")]).collect();
    assert_eq!(
        paths[0],
        Path::new(&fixture.project.runtime.node_path)
            .parent()
            .unwrap()
    );
    assert_eq!(paths.len(), 5);
    assert_eq!(inherited[0].1, "poison");
    assert!(plan.node_command().get_args().next().is_none());
    let summary = serde_json::to_value(plan.summary()).unwrap();
    assert!(summary.get("runtimeRevision").is_some());
    assert!(summary.get("environment").is_none());
    assert!(summary.get("env").is_none());
}

#[test]
fn runtime_preparation_preserves_global_preferences_and_project_files() {
    let fixture = Fixture::new();
    fs::create_dir_all(&fixture.layout.data).unwrap();
    let names = [
        "fnm-enabled",
        "fnm-store.json",
        "php-version",
        "settings.json",
        "sites.json",
    ];
    for name in names {
        fs::write(fixture.layout.data.join(name), name).unwrap();
    }
    for name in [".zshrc", ".zprofile", ".bashrc", ".npmrc"] {
        fs::write(fixture.layout.home.join(name), name).unwrap();
    }
    let before = fs::read(Path::new(&fixture.project.root).join("package.json")).unwrap();
    fixture.prepare().unwrap();
    for name in names {
        assert_eq!(
            fs::read(fixture.layout.data.join(name)).unwrap(),
            name.as_bytes()
        );
    }
    for name in [".zshrc", ".zprofile", ".bashrc", ".npmrc"] {
        assert_eq!(
            fs::read(fixture.layout.home.join(name)).unwrap(),
            name.as_bytes()
        );
    }
    assert_eq!(
        fs::read(Path::new(&fixture.project.root).join("package.json")).unwrap(),
        before
    );
    assert!(!Path::new(&fixture.project.root)
        .join("IMPORTED_SCRIPT_RAN")
        .exists());
}

#[test]
fn inherited_path_and_project_shims_cannot_override_selected_node() {
    let fixture = Fixture::new();
    let directory = Path::new(&fixture.project.root).join("node_modules/.bin");
    fs::create_dir_all(&directory).unwrap();
    for name in ["node", "npm"] {
        fs::write(directory.join(name), "poison").unwrap();
        assert_eq!(
            fixture.prepare().err().unwrap().code,
            ErrorCode::EnvironmentUnavailable
        );
        fs::remove_file(directory.join(name)).unwrap();
    }
}

#[test]
fn invalid_paths_and_unsupported_package_managers_fail_safely() {
    let mut fixture = Fixture::new();
    fixture.project.runtime.node_path = "relative/bin/node".into();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::RuntimeUnavailable
    );
    let fixture = Fixture::new();
    fs::write(Path::new(&fixture.project.root).join("yarn.lock"), "").unwrap();
    assert_eq!(
        fixture.prepare().err().unwrap().code,
        ErrorCode::UnsupportedPackageManager
    );
    assert!(environment::checked_path(Path::new("/tmp/invalid:path")).is_err());
}

fn fixture_inventory(layout: &Layout) -> runtime::Inventory {
    runtime::Inventory {
        choices: runtime::choice(
            layout.runtime.join("v22/bin/node").to_string_lossy().into(),
            "v22.1.0".into(),
            "fnm".into(),
        )
        .into_iter()
        .collect(),
        preferred_id: None,
        error: None,
    }
}

#[test]
fn public_preparation_checks_registration_and_manifest_revisions_before_probing() {
    let fixture = Fixture::new();
    let state = NodeProjects {
        layout: Some(fixture.layout.clone()),
        supported: true,
        mutation: Mutex::new(()),
        discover: fixture_inventory,
    };
    let inspection = state
        .inspect(InspectRequest {
            root: fixture.project.root.clone(),
        })
        .unwrap();
    let saved = state
        .save(SaveRequest {
            config: SaveConfig {
                id: None,
                name: "Saved".into(),
                root: fixture.project.root.clone(),
                script: "dev".into(),
                runtime_id: inspection.runtimes[0].id.clone(),
                open_url: None,
            },
            expected_revision: state.snapshot().revision.unwrap(),
            manifest_revision: inspection.manifest_revision.clone(),
        })
        .unwrap();
    let error = state
        .prepare_runtime(PrepareRuntimeRequest {
            id: saved.project.id.clone(),
            expected_revision: "stale".into(),
            manifest_revision: inspection.manifest_revision.clone(),
        })
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::Conflict);
    let error = state
        .prepare_runtime(PrepareRuntimeRequest {
            id: saved.project.id,
            expected_revision: saved.revision,
            manifest_revision: "stale".into(),
        })
        .err()
        .unwrap();
    assert_eq!(error.code, ErrorCode::Conflict);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "Set FAIRWAY_TEST_NODE_RUNTIMES to two real bin/node paths and run this test alone with --ignored"]
fn real_two_runtime_isolation_and_requirements() {
    // This test owns process-local discovery variables and runs separately from
    // the parallel suite. It changes no actual fnm installation or default.
    let nodes: Vec<_> = std::env::split_paths(
        &std::env::var_os("FAIRWAY_TEST_NODE_RUNTIMES").expect("Set two installed Node paths"),
    )
    .map(|path| fs::canonicalize(path).unwrap())
    .collect();
    assert_eq!(nodes.len(), 2);
    let fixture = Fixture::new();
    let store = fixture.base.join("fnm-store");
    let mut versions = Vec::new();
    for node in &nodes {
        let output = Command::new(node)
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH")
            .args(["-p", "process.version"])
            .output()
            .unwrap();
        assert!(output.status.success());
        versions.push(String::from_utf8(output.stdout).unwrap().trim().to_owned());
    }
    assert_ne!(versions[0], versions[1]);
    fs::create_dir_all(store.join("aliases")).unwrap();
    for (node, version) in nodes.iter().zip(&versions) {
        let parent = store.join("node-versions").join(version);
        fs::create_dir_all(&parent).unwrap();
        std::os::unix::fs::symlink(
            node.parent().unwrap().parent().unwrap(),
            parent.join("installation"),
        )
        .unwrap();
    }
    std::os::unix::fs::symlink(
        nodes[1].parent().unwrap().parent().unwrap(),
        store.join("aliases/default"),
    )
    .unwrap();
    let old = std::env::var_os("FNM_DIR");
    struct Restore(&'static str, Option<OsString>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(value) = &self.1 {
                std::env::set_var(self.0, value);
            } else {
                std::env::remove_var(self.0);
            }
        }
    }
    let _restore = Restore("FNM_DIR", old);
    std::env::set_var("FNM_DIR", &store);
    use crate::node_projects::ChildEnvironment;
    let env_before: ChildEnvironment = std::env::vars_os().collect();
    let default_before = fs::read_link(store.join("aliases/default")).unwrap();
    let state = NodeProjects::new(Some(fixture.layout.clone()));
    for (index, (node, version)) in nodes.iter().zip(&versions).enumerate() {
        let root = fixture.base.join(format!("real-project-{index}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("package.json"),
            br#"{"scripts":{"dev":"touch IMPORTED_SCRIPT_RAN"},"engines":{"node":">=20"}}"#,
        )
        .unwrap();
        fs::write(root.join(".node-version"), version).unwrap();
        let inspection = state
            .inspect(InspectRequest {
                root: root.to_str().unwrap().into(),
            })
            .unwrap();
        let choice = inspection
            .runtimes
            .iter()
            .find(|choice| choice.runtime.node_path == node.to_str().unwrap())
            .expect("Existing fnm inventory must discover this installation");
        let saved = state
            .save(SaveRequest {
                config: SaveConfig {
                    id: None,
                    name: format!("Runtime {index}"),
                    root: inspection.root,
                    script: "dev".into(),
                    runtime_id: choice.id.clone(),
                    open_url: None,
                },
                expected_revision: state.snapshot().revision.unwrap(),
                manifest_revision: inspection.manifest_revision,
            })
            .unwrap();
        let request = || PrepareRuntimeRequest {
            id: saved.project.id.clone(),
            expected_revision: saved.revision.clone(),
            manifest_revision: manifest::inspect(root.to_str().unwrap())
                .unwrap()
                .manifest_revision,
        };
        let old_options = Restore("NODE_OPTIONS", std::env::var_os("NODE_OPTIONS"));
        let old_path = Restore("NODE_PATH", std::env::var_os("NODE_PATH"));
        std::env::set_var("NODE_OPTIONS", "--fairway-invalid-inherited-option");
        std::env::set_var("NODE_PATH", fixture.base.join("poisoned-modules"));
        let plan = state.prepare_runtime(request()).unwrap();
        assert_eq!(&plan.summary().runtime.version, version);
        assert_eq!(
            fs::read_link(store.join("aliases/default")).unwrap(),
            default_before
        );
        assert!(!fixture.layout.data.join("fnm-enabled").exists());
        let output = plan.node_command().args(["-p", "JSON.stringify({version:process.version,node:process.execPath,options:process.env.NODE_OPTIONS||null})"]).output().unwrap();
        assert!(output.status.success());
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["version"], *version);
        assert_eq!(result["node"], node.to_str().unwrap());
        assert!(result["options"].is_null());
        drop(old_options);
        drop(old_path);
        fs::write(
            root.join(".nvmrc"),
            version.trim_start_matches('v').split('.').next().unwrap(),
        )
        .unwrap();
        state.prepare_runtime(request()).unwrap();
        for hint in ["node", "lts/*", "999.0.0"] {
            fs::write(root.join(".nvmrc"), hint).unwrap();
            assert_eq!(
                state.prepare_runtime(request()).err().unwrap().code,
                ErrorCode::RequirementConflict
            );
        }
        fs::remove_file(root.join(".nvmrc")).unwrap();
        fs::write(
            root.join("package.json"),
            br#"{"scripts":{"dev":"touch IMPORTED_SCRIPT_RAN"},"engines":{"node":"<1"}}"#,
        )
        .unwrap();
        assert_eq!(
            state.prepare_runtime(request()).err().unwrap().code,
            ErrorCode::RequirementConflict
        );
        fs::write(
            root.join("package.json"),
            br#"{"scripts":{"dev":"touch IMPORTED_SCRIPT_RAN"},"packageManager":"npm@999.0.0"}"#,
        )
        .unwrap();
        assert_eq!(
            state.prepare_runtime(request()).err().unwrap().code,
            ErrorCode::RequirementConflict
        );
        assert!(!root.join("IMPORTED_SCRIPT_RAN").exists());
    }
    assert_eq!(state.snapshot().projects.len(), 2);
    assert_eq!(
        fs::read_link(store.join("aliases/default")).unwrap(),
        default_before
    );
    assert_eq!(
        std::env::vars_os().collect::<ChildEnvironment>(),
        env_before
    );
    assert!(!fixture.layout.data.join("fnm-enabled").exists());
    assert!(!fixture.layout.data.join("fnm-store.json").exists());
}
