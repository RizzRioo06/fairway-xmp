use super::*;
use std::{fs, path::PathBuf};

struct Fixture {
    base: PathBuf,
    projects: NodeProjects,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!("fw-node-{:032x}", rand::random::<u128>()));
        let layout = Layout::at(base.join("home"), base.join("data"), base.join("runtime"));
        fs::create_dir_all(&layout.runtime).unwrap();
        let executable = layout.runtime.join("node");
        fs::write(&executable, b"This test runtime must never be executed").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut projects = NodeProjects::new(Some(layout));
        projects.supported = true;
        projects.discover = inventory;
        Self { base, projects }
    }
    fn root(&self, name: &str) -> String {
        let root = self.base.join(name);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("package.json"), br#"{"name":"demo","scripts":{"dev":"node server.js","test":"echo ok"},"engines":{"node":">=20"}}"#).unwrap();
        root.to_str().unwrap().into()
    }
    fn inspect(&self, root: &str) -> Inspection {
        self.projects
            .inspect(InspectRequest { root: root.into() })
            .unwrap()
    }
    fn request(&self, root: &str) -> SaveRequest {
        let inspection = self.inspect(root);
        SaveRequest {
            config: SaveConfig {
                id: None,
                name: "Demo 日本".into(),
                root: root.into(),
                script: "dev".into(),
                runtime_id: inspection.runtimes[0].id.clone(),
                open_url: Some("http://127.0.0.1:3000/".into()),
            },
            expected_revision: self.projects.snapshot().revision.unwrap(),
            manifest_revision: inspection.manifest_revision,
        }
    }
    fn path(&self) -> PathBuf {
        self.projects
            .layout()
            .unwrap()
            .data
            .join("node-projects.json")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn inventory(layout: &Layout) -> runtime::Inventory {
    let choice = runtime::choice(
        layout.runtime.join("node").to_string_lossy().into(),
        "v22.1.0".into(),
        "fnm".into(),
    );
    runtime::Inventory {
        preferred_id: choice.as_ref().map(|choice| choice.id.clone()),
        choices: choice.into_iter().collect(),
        error: None,
    }
}

#[test]
fn unicode_registration_round_trips_and_removal_keeps_project_files() {
    let fixture = Fixture::new();
    let root = fixture.root("Student May's 日本 project");
    let response = fixture.projects.save(fixture.request(&root)).unwrap();
    assert_eq!(response.project.name, "Demo 日本");
    let snapshot = fixture.projects.snapshot();
    assert!(snapshot.registry_error.is_none());
    assert_eq!(snapshot.projects.len(), 1);
    assert!(snapshot.projects[0].root_available && snapshot.projects[0].runtime_available);
    let restored = NodeProjects {
        layout: fixture.projects.layout.clone(),
        supported: true,
        mutation: Mutex::new(()),
        discover: inventory,
    };
    assert_eq!(
        restored.snapshot().projects[0].config.id,
        response.project.id
    );
    fixture
        .projects
        .remove(RemoveRequest {
            id: response.project.id,
            expected_revision: response.revision,
        })
        .unwrap();
    assert!(Path::new(&root).join("package.json").is_file());
    assert!(fixture.projects.snapshot().projects.is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(fixture.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn multiple_projects_persist_and_edits_keep_identity_and_other_registrations() {
    let fixture = Fixture::new();
    let first = fixture.root("first");
    let second = fixture.root("second");
    let first_saved = fixture.projects.save(fixture.request(&first)).unwrap();
    let second_saved = fixture.projects.save(fixture.request(&second)).unwrap();
    let mut edit = fixture.request(&first);
    edit.config.id = Some(first_saved.project.id.clone());
    edit.config.name = "Renamed".into();
    edit.config.script = "test".into();
    let edited = fixture.projects.save(edit).unwrap();
    assert_eq!(edited.project.id, first_saved.project.id);
    let snapshot = fixture.projects.snapshot();
    assert_eq!(snapshot.projects.len(), 2);
    assert_eq!(snapshot.projects[0].config.script, "test");
    assert_eq!(snapshot.projects[1].config.id, second_saved.project.id);
    assert_eq!(snapshot.projects[1].config.script, "dev");
}

#[test]
fn malformed_and_oversized_manifests_are_rejected_without_registration() {
    let fixture = Fixture::new();
    let root = fixture.root("bad");
    for bytes in [
        b"{".as_slice(),
        b"[]",
        br#"{"scripts":[]}"#,
        br#"{"scripts":{"dev":12}}"#,
        br#"{"engines":12}"#,
        br#"{"packageManager":false}"#,
        b"\xff",
    ] {
        fs::write(Path::new(&root).join("package.json"), bytes).unwrap();
        assert_eq!(
            fixture
                .projects
                .inspect(InspectRequest { root: root.clone() })
                .unwrap_err()
                .code,
            ErrorCode::InvalidManifest
        );
    }
    fs::write(
        Path::new(&root).join("package.json"),
        vec![b' '; manifest::MANIFEST_LIMIT as usize + 1],
    )
    .unwrap();
    assert_eq!(
        fixture
            .projects
            .inspect(InspectRequest { root })
            .unwrap_err()
            .code,
        ErrorCode::InvalidManifest
    );
    assert!(!fixture.path().exists());
}

#[test]
fn version_hints_are_bounded_and_never_interpreted_as_commands() {
    let fixture = Fixture::new();
    let root = fixture.root("versions");
    let version = Path::new(&root).join(".node-version");
    fs::write(&version, "v22.1.0\n").unwrap();
    assert!(fixture.inspect(&root).suggested_runtime_id.is_some());
    fs::write(Path::new(&root).join(".nvmrc"), "20.1.0").unwrap();
    let inspection = fixture.inspect(&root);
    assert!(inspection.suggested_runtime_id.is_none());
    assert!(inspection
        .warnings
        .iter()
        .any(|warning| warning.contains("different")));
    fs::write(&version, "$(touch never-execute-this)").unwrap();
    assert_eq!(
        fixture.inspect(&root).version_hints.node_version.as_deref(),
        Some("$(touch never-execute-this)")
    );
    assert!(!Path::new(&root).join("never-execute-this").exists());
    fs::write(&version, vec![b'1'; 4097]).unwrap();
    assert_eq!(
        fixture
            .projects
            .inspect(InspectRequest { root: root.clone() })
            .unwrap_err()
            .code,
        ErrorCode::InvalidManifest
    );
    fs::write(&version, b"\xff").unwrap();
    assert_eq!(
        fixture
            .projects
            .inspect(InspectRequest { root })
            .unwrap_err()
            .code,
        ErrorCode::InvalidManifest
    );
}

#[test]
fn duplicate_roots_and_symlink_aliases_are_rejected() {
    let fixture = Fixture::new();
    let root = fixture.root("original");
    fixture.projects.save(fixture.request(&root)).unwrap();
    assert_eq!(
        fixture
            .projects
            .save(fixture.request(&root))
            .unwrap_err()
            .code,
        ErrorCode::InvalidProject
    );
    #[cfg(unix)]
    {
        let alias = fixture.base.join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        assert_eq!(
            fixture
                .projects
                .save(fixture.request(alias.to_str().unwrap()))
                .unwrap_err()
                .code,
            ErrorCode::InvalidProject
        );
    }
    assert_eq!(fixture.projects.snapshot().projects.len(), 1);
}

#[test]
fn missing_runtime_blocks_save_and_is_visible_after_registration() {
    let fixture = Fixture::new();
    let root = fixture.root("missing-runtime");
    let request = fixture.request(&root);
    let saved = fixture.projects.save(fixture.request(&root)).unwrap();
    fs::remove_file(fixture.projects.layout().unwrap().runtime.join("node")).unwrap();
    assert!(!fixture.projects.snapshot().projects[0].runtime_available);
    let mut request = request;
    request.config.id = Some(saved.project.id);
    request.expected_revision = saved.revision;
    assert_eq!(
        fixture.projects.save(request).unwrap_err().code,
        ErrorCode::RuntimeUnavailable
    );
    assert_eq!(fixture.projects.snapshot().projects.len(), 1);
}

#[test]
fn stale_registry_and_manifest_revisions_preserve_saved_configuration() {
    let fixture = Fixture::new();
    let root = fixture.root("stale");
    let stale = fixture.request(&root);
    fixture.projects.save(fixture.request(&root)).unwrap();
    let saved = fs::read(fixture.path()).unwrap();
    assert_eq!(
        fixture.projects.save(stale).unwrap_err().code,
        ErrorCode::Conflict
    );
    let mut edit = fixture.request(&root);
    edit.config.id = Some(fixture.projects.snapshot().projects[0].config.id.clone());
    fs::write(Path::new(&root).join(".nvmrc"), "22.1.0").unwrap();
    assert_eq!(
        fixture.projects.save(edit).unwrap_err().code,
        ErrorCode::Conflict
    );
    assert_eq!(fs::read(fixture.path()).unwrap(), saved);
}

#[test]
fn corrupt_and_future_registries_are_preserved_on_reads_and_mutations() {
    let fixture = Fixture::new();
    let root = fixture.root("corrupt");
    let request = fixture.request(&root);
    fs::create_dir_all(fixture.path().parent().unwrap()).unwrap();
    for bytes in [
        b"not json".as_slice(),
        br#"{"schemaVersion":2,"projects":[]}"#,
        br#"{"schemaVersion":1,"projects":[],"future":true}"#,
    ] {
        fs::write(fixture.path(), bytes).unwrap();
        let snapshot = fixture.projects.snapshot();
        assert!(snapshot.registry_error.is_some());
        assert!(snapshot.revision.is_none());
        assert_eq!(fs::read(fixture.path()).unwrap(), bytes);
        assert_eq!(
            fixture
                .projects
                .remove(RemoveRequest {
                    id: "missing".into(),
                    expected_revision: "anything".into()
                })
                .unwrap_err()
                .code,
            ErrorCode::RegistryUnavailable
        );
    }
    assert_eq!(
        fixture.projects.save(request).unwrap_err().code,
        ErrorCode::RegistryUnavailable
    );
}

#[test]
fn missing_roots_are_reported_and_can_be_removed() {
    let fixture = Fixture::new();
    let root = fixture.root("moved");
    let saved = fixture.projects.save(fixture.request(&root)).unwrap();
    fs::remove_dir_all(&root).unwrap();
    let snapshot = fixture.projects.snapshot();
    assert!(!snapshot.projects[0].root_available && !snapshot.projects[0].manifest_available);
    fixture
        .projects
        .remove(RemoveRequest {
            id: saved.project.id,
            expected_revision: saved.revision,
        })
        .unwrap();
}

#[test]
fn inspection_and_registration_never_execute_scripts_or_modify_php_and_fnm_files() {
    let fixture = Fixture::new();
    let root = fixture.root("no-execution");
    fs::write(
        Path::new(&root).join("package.json"),
        br#"{"scripts":{"dev":"touch executed","predev":"touch executed"}}"#,
    )
    .unwrap();
    let data = &fixture.projects.layout().unwrap().data;
    fs::create_dir_all(data).unwrap();
    let names = [
        "sites.json",
        "settings.json",
        "php-version",
        "fnm-store.json",
        "fnm-enabled",
    ];
    for name in names {
        fs::write(data.join(name), name).unwrap();
    }
    fixture.projects.save(fixture.request(&root)).unwrap();
    assert!(!Path::new(&root).join("executed").exists());
    for name in names {
        assert_eq!(fs::read(data.join(name)).unwrap(), name.as_bytes());
    }
    assert_eq!(
        fs::read(fixture.projects.layout().unwrap().runtime.join("node")).unwrap(),
        b"This test runtime must never be executed"
    );
}

#[test]
fn manager_hints_do_not_silently_claim_npm_support() {
    let fixture = Fixture::new();
    let root = fixture.root("pnpm");
    fs::write(Path::new(&root).join("pnpm-lock.yaml"), "").unwrap();
    assert_eq!(fixture.inspect(&root).package_manager.name, "pnpm");
    assert!(!fixture.inspect(&root).package_manager.npm_supported);
    fs::write(Path::new(&root).join("package-lock.json"), "").unwrap();
    assert_eq!(fixture.inspect(&root).package_manager.name, "unknown");
    assert!(!fixture.inspect(&root).package_manager.npm_supported);
    fixture.projects.save(fixture.request(&root)).unwrap();
}

#[test]
fn contracts_use_camel_case_and_reject_unexpected_input_fields() {
    let fixture = Fixture::new();
    let root = fixture.root("contract");
    let json = serde_json::to_value(fixture.inspect(&root)).unwrap();
    assert!(json.get("manifestRevision").is_some());
    assert!(json["runtimes"][0].get("nodePath").is_some());
    assert!(json["versionHints"].get("enginesNode").is_some());
    assert_eq!(
        serde_json::to_value(Error::new(ErrorCode::RuntimeUnavailable, "missing")).unwrap()["code"],
        "runtimeUnavailable"
    );
    assert!(serde_json::from_value::<InspectRequest>(
        serde_json::json!({"root":root,"command":"execute me"})
    )
    .is_err());
}

#[test]
fn unsupported_platform_and_missing_layout_are_explicit_and_do_not_create_files() {
    let fixture = Fixture::new();
    let mut state = NodeProjects::new(fixture.projects.layout.clone());
    state.supported = false;
    assert!(!state.snapshot().supported);
    assert_eq!(
        state
            .inspect(InspectRequest {
                root: fixture.root("unsupported")
            })
            .unwrap_err()
            .code,
        ErrorCode::UnsupportedPlatform
    );
    state.supported = true;
    state.layout = None;
    assert_eq!(
        state.snapshot().registry_error.unwrap().code,
        ErrorCode::Unavailable
    );
    assert!(!fixture.path().exists());
}

#[test]
fn open_urls_are_restricted_to_loopback_without_credentials() {
    for url in [
        "http://localhost:3000/",
        "https://127.0.0.1/",
        "http://[::1]:8080/a?b=c",
    ] {
        validate_url(Some(url)).unwrap();
    }
    for url in [
        "file:///tmp/demo",
        "http://localhost.evil/",
        "http://localhost@evil/",
        "http://localhost:0/",
        "http://localhost:65536/",
        "http://127.0.0.1\\@evil/",
        "http://localhost/\n",
    ] {
        assert!(validate_url(Some(url)).is_err(), "{url}");
    }
}

#[cfg(unix)]
#[test]
fn symlinked_and_special_inspection_files_are_rejected_without_blocking() {
    let fixture = Fixture::new();
    let root = fixture.root("special");
    let path = Path::new(&root).join("package.json");
    fs::rename(&path, Path::new(&root).join("real.json")).unwrap();
    std::os::unix::fs::symlink("real.json", &path).unwrap();
    assert_eq!(
        fixture
            .projects
            .inspect(InspectRequest { root: root.clone() })
            .unwrap_err()
            .code,
        ErrorCode::InvalidManifest
    );
    fs::remove_file(&path).unwrap();
    use std::os::unix::ffi::OsStrExt;
    let fifo = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert_eq!(
        fixture
            .projects
            .inspect(InspectRequest { root })
            .unwrap_err()
            .code,
        ErrorCode::InvalidManifest
    );
}
