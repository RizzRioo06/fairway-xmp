use super::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Default)]
struct MemoryVault {
    values: Mutex<HashMap<String, Vec<u8>>>,
    fail_create: AtomicBool,
    fail_remove: AtomicBool,
}
impl Vault for MemoryVault {
    fn create(&self, id: &str, p: &Password) -> Result<()> {
        if self.fail_create.load(Ordering::SeqCst) {
            return Err(fail(Code::Unavailable, "Vault unavailable"));
        }
        self.values
            .lock()
            .unwrap()
            .insert(id.into(), p.bytes().to_vec());
        Ok(())
    }
    fn read(&self, id: &str) -> Result<Password> {
        Password::from_bytes(
            self.values
                .lock()
                .unwrap()
                .get(id)
                .ok_or_else(|| fail(Code::Unavailable, "Missing credential"))?,
        )
    }
    fn remove(&self, id: &str) -> Result<()> {
        if self.fail_remove.load(Ordering::SeqCst) {
            return Err(fail(Code::Unavailable, "Vault unavailable"));
        }
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}
struct Fixture {
    path: PathBuf,
    store: Store,
    vault: Arc<MemoryVault>,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fairway-pg-profiles-{:032x}",
            rand::random::<u128>()
        ));
        std::fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = std::fs::canonicalize(path).unwrap();
        let vault = Arc::new(MemoryVault::default());
        let store = Store {
            data: Some(path.clone()),
            gate: Mutex::new(()),
            vault: vault.clone(),
        };
        Self { path, store, vault }
    }
    fn save(&self, profile: Profile, password: Option<Password>) -> Result<Profile> {
        self.store.save(Save {
            confirmed: true,
            profile,
            password,
        })
    }
    fn bytes(&self) -> Vec<u8> {
        std::fs::read(self.path.join("postgresql-connections/profiles.json")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).unwrap();
    }
}
fn profile() -> Profile {
    Profile {
        id: "a".repeat(32),
        revision: "".into(),
        name: "Local development".into(),
        host: "127.0.0.1".into(),
        port: 5432,
        user: "student".into(),
        database: "practice 日本".into(),
        remember_password: true,
    }
}
fn password() -> Password {
    Password::from_bytes(b"fixture-password-marker").unwrap()
}
#[test]
fn profiles_survive_reopen_without_serializing_credentials_and_support_rename_and_forget() {
    let f = Fixture::new();
    let mut p = f.save(profile(), Some(password())).unwrap();
    assert!(!String::from_utf8(f.bytes())
        .unwrap()
        .contains("fixture-password-marker"));
    let reopened = Store {
        data: Some(f.path.clone()),
        gate: Mutex::new(()),
        vault: f.vault.clone(),
    };
    assert_eq!(reopened.list().unwrap()[0].name, p.name);
    let connection = reopened
        .connection(Open {
            confirmed: true,
            id: p.id.clone(),
            revision: p.revision.clone(),
            password: None,
        })
        .unwrap();
    assert_eq!(connection.database, p.database);
    if let Target::External { password, .. } = connection.target {
        assert!(password.bytes() == b"fixture-password-marker");
    } else {
        panic!("wrong target");
    }
    p.name = "Renamed".into();
    let p = f.save(p, None).unwrap();
    assert_eq!(f.store.list().unwrap()[0].name, "Renamed");
    assert_eq!(f.vault.values.lock().unwrap().len(), 1);
    f.store
        .remove(Remove {
            confirmed: true,
            id: p.id,
            revision: p.revision,
        })
        .unwrap();
    assert!(f.store.list().unwrap().is_empty());
    assert!(f.vault.values.lock().unwrap().is_empty());
}
#[test]
fn stale_edits_and_missing_consent_do_not_replace_data() {
    let f = Fixture::new();
    let original = profile();
    let p = f.save(original.clone(), Some(password())).unwrap();
    assert!(f.save(original, Some(password())).is_err());
    assert!(f
        .store
        .remove(Remove {
            confirmed: false,
            id: p.id,
            revision: p.revision
        })
        .is_err());
    assert_eq!(f.store.list().unwrap().len(), 1);
}
#[test]
fn failed_vault_write_preserves_previous_connection_and_cleanup_is_recoverable() {
    let f = Fixture::new();
    let p = f.save(profile(), Some(password())).unwrap();
    f.vault.fail_create.store(true, Ordering::SeqCst);
    assert!(f
        .save(
            p.clone(),
            Some(Password::from_bytes(b"replacement").unwrap())
        )
        .is_err());
    assert_eq!(f.store.list().unwrap()[0].revision, p.revision);
    assert_eq!(f.vault.values.lock().unwrap().len(), 1);
    f.vault.fail_create.store(false, Ordering::SeqCst);
    let p = f.save(p, None).unwrap();
    f.vault.fail_remove.store(true, Ordering::SeqCst);
    assert!(f
        .store
        .remove(Remove {
            confirmed: true,
            id: p.id,
            revision: p.revision
        })
        .is_err());
    assert!(f.store.list().unwrap().is_empty());
    f.vault.fail_remove.store(false, Ordering::SeqCst);
    let mut p = profile();
    p.remember_password = false;
    f.save(p, None).unwrap();
    assert!(f.vault.values.lock().unwrap().is_empty());
}
#[test]
fn forgetting_password_requires_entry_next_time_and_override_never_persists() {
    let f = Fixture::new();
    let mut p = f.save(profile(), Some(password())).unwrap();
    p.remember_password = false;
    let p = f.save(p, None).unwrap();
    assert!(f.vault.values.lock().unwrap().is_empty());
    assert!(f
        .store
        .connection(Open {
            confirmed: true,
            id: p.id.clone(),
            revision: p.revision.clone(),
            password: None
        })
        .is_err());
    assert!(f
        .store
        .connection(Open {
            confirmed: true,
            id: p.id,
            revision: p.revision,
            password: Some(password())
        })
        .is_ok());
    assert!(!String::from_utf8(f.bytes())
        .unwrap()
        .contains("fixture-password-marker"));
}
#[test]
fn unsafe_metadata_aliases_and_corrupt_documents_are_preserved_and_refused() {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let mut p = profile();
    p.remember_password = false;
    f.save(p, None).unwrap();
    let file = f.path.join("postgresql-connections/profiles.json");
    let outside = f.path.join("unrelated");
    std::fs::rename(&file, &outside).unwrap();
    #[cfg(unix)]
    {
        symlink(&outside, &file).unwrap();
        assert!(f.store.list().is_err());
        assert!(outside.is_file());
        std::fs::remove_file(&file).unwrap();
    }
    std::fs::hard_link(&outside, &file).unwrap();
    assert!(f.store.list().is_err());
    std::fs::remove_file(&file).unwrap();
    std::fs::write(&file, b"invalid json").unwrap();
    assert!(f.store.list().is_err());
    assert_eq!(std::fs::read(&file).unwrap(), b"invalid json");
}

#[test]
fn interrupted_credential_creation_is_recovered_without_deleting_active_passwords() {
    let f = Fixture::new();
    let p = f.save(profile(), Some(password())).unwrap();
    let orphan = "c".repeat(32);
    f.store
        .with(|dir, j| {
            j.retired.push(orphan.clone());
            Store::commit(dir, j)
        })
        .unwrap();
    f.vault.create(&orphan, &password()).unwrap();
    assert_eq!(f.vault.values.lock().unwrap().len(), 2);
    let p = f.save(p, None).unwrap();
    assert_eq!(f.vault.values.lock().unwrap().len(), 1);
    assert!(f
        .store
        .connection(Open {
            confirmed: true,
            id: p.id,
            revision: p.revision,
            password: None
        })
        .is_ok());
}
#[test]
fn changing_server_identity_requires_new_password_and_stale_profiles_cannot_connect() {
    let f = Fixture::new();
    let p = f.save(profile(), Some(password())).unwrap();
    let mut changed = p.clone();
    changed.port = 5433;
    assert!(f.save(changed.clone(), None).is_err());
    let updated = f.save(changed, Some(password())).unwrap();
    assert!(f
        .store
        .connection(Open {
            confirmed: true,
            id: p.id,
            revision: p.revision,
            password: None
        })
        .is_err());
    assert!(f
        .store
        .connection(Open {
            confirmed: true,
            id: updated.id,
            revision: updated.revision,
            password: None
        })
        .is_ok());
}
#[test]
fn profile_validation_rejects_remote_hosts_invalid_identifiers_and_unknown_fields() {
    let f = Fixture::new();
    for host in ["remote.example", "localhost", "", "127.1"] {
        let mut p = profile();
        p.host = host.into();
        assert!(f.save(p, Some(password())).is_err());
    }
    let mut p = profile();
    p.user = "日".repeat(22);
    assert!(f.save(p, Some(password())).is_err());
    let mut p = profile();
    p.database = "bad\0name".into();
    assert!(f.save(p, Some(password())).is_err());
    let mut p = profile();
    p.name = " ".into();
    assert!(f.save(p, Some(password())).is_err());
    let mut value = serde_json::to_value(profile()).unwrap();
    value["password"] = serde_json::json!("forbidden");
    assert!(serde_json::from_value::<Profile>(value).is_err());
}
