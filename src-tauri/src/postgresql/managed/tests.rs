use super::*;
use std::{
    collections::BTreeMap,
    fs,
    net::TcpListener,
    os::unix::fs::{symlink, PermissionsExt},
};
#[derive(Default)]
struct MemoryVault(Mutex<BTreeMap<String, Secret>>);
impl Vault for MemoryVault {
    fn create(&self, id: &str, s: &Secret) -> Result<()> {
        let mut m = self.0.lock().unwrap();
        if m.contains_key(id) {
            return Err(failure("Duplicate fixture secret"));
        }
        m.insert(id.into(), Secret(s.0.clone()));
        Ok(())
    }
    fn read(&self, id: &str) -> Result<Secret> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .map(|s| Secret(s.0.clone()))
            .ok_or_else(|| failure("Fixture secret unavailable"))
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("fw-pg2-{:032x}", rand::random::<u128>()));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Self(fs::canonicalize(p).unwrap())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Retained isolated PostgreSQL fixture: {}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Scratch, Controller, Arc<MemoryVault>) {
    let s = Scratch::new();
    let v = Arc::new(MemoryVault::default());
    let l = Layout::at(s.0.join("home"), s.0.clone(), s.0.join("tools"));
    let c = Controller::open(&l, v.clone()).unwrap();
    (s, c, v)
}
#[test]
fn private_operations_refuse_links_foreign_publication_and_changed_identity() {
    let s = Scratch::new();
    let d = Dir::absolute(&s.0).unwrap();
    d.private().unwrap();
    let stage = d.make("stage").unwrap();
    stage.create("sentinel", b"kept", false).unwrap();
    d.make("occupied").unwrap();
    assert!(d.publish("stage", "occupied").is_err());
    assert_eq!(stage.read("sentinel", 100).unwrap(), b"kept");
    symlink(s.0.join("stage"), s.0.join("link")).unwrap();
    assert!(d.child("link").is_err());
    assert!(d.read("link", 100).is_err());
    fs::rename(s.0.join("stage"), s.0.join("old")).unwrap();
    d.make("stage").unwrap();
    assert!(stage.assert_path(&s.0.join("stage")).is_err());
    let lease = d.lock("lease").unwrap();
    assert!(d.lock("lease").is_err());
    drop(lease);
    d.lock("lease").unwrap();
    fs::hard_link(s.0.join("old/sentinel"), s.0.join("hardlink")).unwrap();
    assert!(d.read("hardlink", 100).is_err());
    assert!(d.entries().is_err());
}
#[test]
fn consent_constructor_and_read_only_requests_never_initialize_data() {
    let s = Scratch::new();
    let layout = Layout::at(s.0.join("home"), s.0.clone(), s.0.join("tools"));
    let m = Managed::new(Some(layout));
    assert!(m
        .initialize(Initialize {
            confirmed: false,
            port: 15432
        })
        .is_err());
    assert!(!s.0.join("postgresql").exists());
    assert!(m.provision(Confirmation { confirmed: false }).is_err());
    assert!(!m.snapshot().unwrap().running);
    assert!(!m.health().unwrap().running);
    assert!(m.logs().unwrap().text.is_empty());
    assert!(!s.0.join("postgresql").exists());
}
#[test]
fn foreign_root_and_reserved_ports_are_refused_without_shared_settings_migration() {
    let s = Scratch::new();
    let l = Layout::at(s.0.join("home"), s.0.clone(), s.0.join("tools"));
    let data = Dir::absolute(&s.0).unwrap();
    let foreign = data.make("postgresql").unwrap();
    foreign.create("PG_VERSION", b"18\n", false).unwrap();
    assert!(Controller::open(&l, Arc::new(MemoryVault::default())).is_err());
    assert!(!foreign.exists("service.lock").unwrap());
    assert_eq!(foreign.read("PG_VERSION", 100).unwrap(), b"18\n");
    let (_s, c, _) = fixture();
    assert!(c.check_port(13306).is_err());
    assert!(c.check_port(crate::tls::https_port()).is_err());
    let old = br#"{"website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000}"#;
    fs::write(c.settings.join("settings.json"), old).unwrap();
    assert!(c.check_port(13306).is_err());
    assert_eq!(fs::read(c.settings.join("settings.json")).unwrap(), old);
}
#[test]
fn malformed_pin_and_existing_unowned_runtime_are_never_adopted() {
    let (_s, c, _v) = fixture();
    c.base.make("runtime-18.6").unwrap();
    assert!(c.runtime().is_err());
    assert!(runtime::provision(&c.base, &c.path, &c.source).is_err());
}
#[test]
fn bounded_logs_never_retain_unbounded_output() {
    let s = Scratch::new();
    let file = files::log_file(&s.0.join("log")).unwrap();
    let mut cap = logs::Capture::new(file);
    cap.test_append(&vec![b'x'; 2 * 1024 * 1024]);
    let out = cap.read();
    assert!(out.truncated);
    assert_eq!(out.text.len(), 64 * 1024);
    assert!(fs::metadata(s.0.join("log")).unwrap().len() <= 1024 * 1024);
}
#[test]
#[ignore = "Requires the explicitly provisioned pinned payload; uses memory-only fixture vault, never login Keychain"]
fn real_pinned_runtime_scram_lifecycle_health_integrity_and_independence() {
    let (_s, mut c, vault) = fixture();
    c.source = std::env::var_os("FAIRWAY_TEST_POSTGRES_PAYLOAD")
        .expect("Provide isolated pinned payload")
        .into();
    runtime::provision(&c.base, &c.path, &c.source).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let occupied = listener.local_addr().unwrap().port();
    assert!(c.initialize(occupied).is_err());
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    struct DeniedVault;
    impl Vault for DeniedVault {
        fn create(&self, _: &str, _: &Secret) -> Result<()> {
            Err(failure("Fixture vault unavailable"))
        }
        fn read(&self, _: &str) -> Result<Secret> {
            Err(failure("Fixture vault unavailable"))
        }
    }
    let (_denied_scratch, mut denied, _) = fixture();
    denied.source = c.source.clone();
    runtime::provision(&denied.base, &denied.path, &denied.source).unwrap();
    denied.vault = Arc::new(DeniedVault);
    assert!(denied.initialize(port).is_err());
    let entries = denied.base.entries().unwrap();
    assert!(!entries
        .iter()
        .any(|(n, _)| n.starts_with(".cluster-") || n.starts_with("cluster-")));
    let job = entries.iter().find(|(n, _)| n.starts_with("job-")).unwrap();
    let intent: serde_json::Value = decode(&denied.base.read(&job.0, 4096).unwrap()).unwrap();
    assert_eq!(intent["phase"], "securingCredential");
    denied.vault = vault.clone();
    assert!(denied.initialize(port).is_err()); // Retained intent never auto-retried.
    c.initialize(port).unwrap();
    assert!(c.run.is_none());
    let id = c.cluster.as_ref().unwrap().id.clone();
    let path = c.cluster_path(c.cluster.as_ref().unwrap()).unwrap();
    assert_eq!(fs::read(path.join("PG_VERSION")).unwrap(), b"18\n");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(c.initialize(port).is_err());
    c.start().unwrap();
    // A private worker must ignore inherited libpq service configuration.
    std::env::set_var("PGSERVICE", "fairway-fixture-must-never-be-loaded");
    let status = c.snapshot(true).unwrap();
    std::env::remove_var("PGSERVICE");
    assert_eq!(status.authentication, "authenticated");
    assert_eq!(status.responsiveness, "accepting");
    assert!(listener.local_addr().is_ok());
    assert!(c.base.lock("runtime.lease").is_err());
    assert!(c.start().is_err());
    let secret = {
        let mut values = vault.0.lock().unwrap();
        let (_, value) = values.iter_mut().next().unwrap();
        let old = Secret(value.0.clone());
        *value = Secret::random();
        old
    };
    let rejected = c.snapshot(true).unwrap();
    assert_eq!(rejected.responsiveness, "accepting");
    assert_eq!(rejected.authentication, "rejected");
    {
        let mut values = vault.0.lock().unwrap();
        *values.values_mut().next().unwrap() = Secret(secret.0.clone());
    }
    // A real authenticated client holds a session. Smart shutdown must wait,
    // retain ownership/lease and avoid a forced database kill.
    use std::io::Write;
    let (read, mut write) = std::os::unix::net::UnixStream::pair().unwrap();
    let fd: std::os::fd::OwnedFd = read.into();
    write.write_all(&secret.0).unwrap();
    write.write_all(b"\n").unwrap();
    write.shutdown(std::net::Shutdown::Write).unwrap();
    let rt = c.runtime().unwrap();
    let mut client = clean_command(rt.path.join("bin/psql"));
    let mut client = client
        .args([
            "-X",
            "-W",
            "-h",
            "127.0.0.1",
            "-U",
            "fairway",
            "-d",
            "postgres",
            "-p",
        ])
        .arg(port.to_string())
        .args(["-c", "SELECT pg_sleep(15)"])
        .stdin(Stdio::from(fd))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(write);
    std::thread::sleep(Duration::from_millis(750));
    assert!(client.try_wait().unwrap().is_none());
    assert!(
        c.stop().is_err(),
        "Smart shutdown must wait for the authenticated fixture client"
    );
    assert!(c.run.is_some());
    assert!(c.base.lock("runtime.lease").is_err());
    // Let the bounded query complete normally; closing a client socket alone
    // does not cancel a running PostgreSQL statement.
    client.wait().unwrap();
    c.stop().unwrap();
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    c.start().unwrap();
    assert_eq!(c.snapshot(true).unwrap().authentication, "authenticated");
    c.stop().unwrap();
    fs::write(
        path.join("postgresql.auto.conf"),
        "listen_addresses = '*'\n",
    )
    .unwrap();
    assert!(c.start().is_err());
    fs::write(path.join("postgresql.auto.conf"), "# preserved\n").unwrap();
    fs::write(path.join("postmaster.pid"), b"1\n").unwrap();
    assert!(c.start().is_err());
    assert_eq!(fs::read(path.join("postmaster.pid")).unwrap(), b"1\n");
    fs::remove_file(path.join("postmaster.pid")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(c.start().is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let logs = c.logs().unwrap();
    assert!(!logs.text.contains(std::str::from_utf8(&secret.0).unwrap()));
    for (name, mode) in c.base.entries().unwrap() {
        if mode & libc::S_IFMT as u32 == libc::S_IFREG as u32 {
            let bytes = c.base.read(&name, 1024 * 1024).unwrap();
            assert!(
                !bytes.windows(secret.0.len()).any(|w| w == secret.0),
                "Plain credential found in retained metadata/log"
            );
        }
    }
    assert_eq!(c.cluster.as_ref().unwrap().id, id);
    assert!(listener.local_addr().is_ok());
    let layout = Layout::at(
        c.settings.join("home"),
        c.settings.clone(),
        c.source.parent().unwrap().to_owned(),
    );
    drop(c);
    let mut c = Controller::open(&layout, vault.clone()).unwrap();
    assert_eq!(c.cluster.as_ref().unwrap().id, id);
    assert!(!c.logs().unwrap().text.is_empty());
    c.start().unwrap();
    c.stop().unwrap();
    // A published cluster without registration is retained, never silently adopted.
    fs::rename(
        c.path.join("active.json"),
        c.path.join("receipt-retained.json"),
    )
    .unwrap();
    c.cluster = None;
    assert!(c.initialize(port).is_err());
    fs::rename(
        c.path.join("receipt-retained.json"),
        c.path.join("active.json"),
    )
    .unwrap();
    drop(c);
    let mut c = Controller::open(&layout, vault).unwrap();
    let rt = c.runtime().unwrap();
    let bytes = fs::read(rt.path.join("bin/pg_ctl")).unwrap();
    fs::set_permissions(
        rt.path.join("bin/pg_ctl"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::write(rt.path.join("bin/pg_ctl"), &bytes[..bytes.len() - 1]).unwrap();
    assert!(c.start().is_err());
}

#[test]
fn generic_data_removal_preserves_postgresql_directories_and_links() {
    let s = Scratch::new();
    assert!(crate::postgresql::ensure_data_removal_allowed(&s.0).is_ok());
    let d = Dir::absolute(&s.0).unwrap();
    let pg = d.make("postgresql").unwrap();
    pg.create("PG_VERSION", b"18\n", false).unwrap();
    assert!(crate::postgresql::ensure_data_removal_allowed(&s.0).is_err());
    assert_eq!(pg.read("PG_VERSION", 100).unwrap(), b"18\n");
    fs::rename(s.0.join("postgresql"), s.0.join("kept")).unwrap();
    symlink(s.0.join("kept"), s.0.join("postgresql")).unwrap();
    assert!(crate::postgresql::ensure_data_removal_allowed(&s.0).is_err());
}
