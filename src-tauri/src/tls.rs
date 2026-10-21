//! Local HTTPS material for the optional project sites.
use crate::config::{atomic_write, private_file, Layout, Result};
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, Issuer, KeyPair, KeyUsagePurpose,
    NameConstraints,
};
use std::{fs, path::PathBuf, process::Command, time::Duration};
use time::OffsetDateTime;

mod policy;

pub const HTTPS_PORT: u16 = 8443;
const CA_NAME: &str = "Fairway XMP Local CA";

#[cfg(all(target_os = "macos", feature = "native-macos-trust"))]
mod macos_native;

pub fn https_port() -> u16 {
    #[cfg(debug_assertions)]
    if let Ok(port) = std::env::var("FAIRWAY_DEV_HTTPS_PORT") {
        if let Ok(port) = port.parse::<u16>() {
            if port >= 1024 {
                return port;
            }
        }
    }
    HTTPS_PORT
}

fn state_directory(layout: &Layout) -> PathBuf {
    layout.data.join("tls")
}
fn directory(layout: &Layout) -> PathBuf {
    let base = state_directory(layout);
    if base.join("active-v2").is_file() {
        base.join("upgrade/tls")
    } else {
        base
    }
}

pub fn certificate(layout: &Layout) -> PathBuf {
    directory(layout).join("projects.pem")
}

pub fn private_key(layout: &Layout) -> PathBuf {
    directory(layout).join("projects-key.pem")
}

fn root_certificate(layout: &Layout) -> PathBuf {
    directory(layout).join("root-ca.pem")
}

fn root_key(layout: &Layout) -> PathBuf {
    directory(layout).join("root-ca-key.pem")
}

fn trust_marker(layout: &Layout) -> PathBuf {
    state_directory(layout).join("trusted")
}

pub fn trusted(layout: &Layout) -> bool {
    trust_marker(layout).exists()
}

/// Remove saved HTTPS choices so a kept profile does not serve untrusted certificates.
fn clear_https(layout: &Layout) -> Result<()> {
    let mut projects = layout.registered_projects()?;
    if projects.iter().any(|project| project.https) {
        projects
            .iter_mut()
            .for_each(|project| project.https = false);
        layout.save_projects(&projects)?;
    }
    Ok(())
}

fn ca_params() -> CertificateParams {
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params.distinguished_name.push(DnType::CommonName, CA_NAME);
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![GeneralSubtree::DnsName(".test".into())],
        excluded_subtrees: vec![
            GeneralSubtree::IpAddress("0.0.0.0/0".parse().unwrap()),
            GeneralSubtree::IpAddress("::/0".parse().unwrap()),
        ],
    });
    params.not_before = OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_after = OffsetDateTime::now_utc() + time::Duration::days(3650);
    params
}

struct MaterialLock(std::fs::File);
impl MaterialLock {
    fn acquire(layout: &Layout) -> Result<Self> {
        let dir = state_directory(layout);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            options.mode(0o600);
        }
        let file = options
            .open(dir.join("material.lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock().map_err(|_| {
            "Another HTTPS setup is in progress. Wait for it to finish, then retry.".to_string()
        })?;
        Ok(Self(file))
    }
}
impl Drop for MaterialLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub fn ensure_certificate(layout: &Layout) -> Result<()> {
    let _lock = MaterialLock::acquire(layout)?;
    ensure_certificate_inner(layout)
}
fn ensure_certificate_inner(layout: &Layout) -> Result<()> {
    let dir = directory(layout);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let ca_cert = root_certificate(layout);
    let ca_key = root_key(layout);
    if ca_cert.exists() != ca_key.exists() {
        return Err(
            "The local HTTPS certificate authority is incomplete. Its files were preserved.".into(),
        );
    }
    if !ca_cert.exists() {
        let key = KeyPair::generate().map_err(|e| e.to_string())?;
        let ca = CertifiedIssuer::self_signed(ca_params(), &key).map_err(|e| e.to_string())?;
        atomic_write(&ca_key, key.serialize_pem().as_bytes())?;
        private_file(&ca_key)?;
        atomic_write(&ca_cert, ca.pem().as_bytes())?;
        atomic_write(&dir.join("root-ca.der"), ca.der().as_ref())?;
    }
    let ca_bytes = policy::read_certificate(&ca_cert)?;
    policy::validate(&ca_bytes, true, false)?;
    let ca_pair = KeyPair::from_pem(&fs::read_to_string(&ca_key).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    policy::key_matches(&ca_bytes, &ca_pair)?;
    let mut domains = layout
        .registered_projects()?
        .into_iter()
        .filter(|project| project.https)
        .map(|project| format!("{}.test", project.domain))
        .collect::<Vec<_>>();
    domains.push("fairway.test".into());
    domains.sort();
    domains.dedup();
    let domains_path = dir.join("certificate-domains.json");
    let matching_domains = fs::read(&domains_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .is_some_and(|saved| saved == domains);
    let recent = matching_domains
        && [certificate(layout), private_key(layout)]
            .iter()
            .all(|path| {
                fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .and_then(|modified| modified.elapsed().map_err(std::io::Error::other))
                    .is_ok_and(|age| age < Duration::from_secs(300 * 24 * 3600))
            });
    if recent
        && fs::read(certificate(layout))
            .ok()
            .zip(fs::read_to_string(private_key(layout)).ok())
            .is_some_and(|(cert, key)| policy::leaf_matches(&cert, &key, &ca_bytes, &domains))
    {
        return Ok(());
    }
    let key = KeyPair::from_pem(&fs::read_to_string(&ca_key).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let issuer = Issuer::from_ca_cert_pem(
        &fs::read_to_string(&ca_cert).map_err(|e| e.to_string())?,
        &key,
    )
    .map_err(|e| e.to_string())?;
    let server_key = KeyPair::generate().map_err(|e| e.to_string())?;
    let mut params = CertificateParams::new(domains.clone()).map_err(|e| e.to_string())?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "Fairway XMP project sites");
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.not_before = OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_after = OffsetDateTime::now_utc() + time::Duration::days(365);
    let server = params
        .signed_by(&server_key, &issuer)
        .map_err(|e| e.to_string())?;
    atomic_write(&private_key(layout), server_key.serialize_pem().as_bytes())?;
    private_file(&private_key(layout))?;
    atomic_write(&certificate(layout), server.pem().as_bytes())?;
    atomic_write(
        &domains_path,
        &serde_json::to_vec(&domains).map_err(|e| e.to_string())?,
    )?;
    crate::durability::sync_directory(&dir)?;
    crate::durability::sync_directory(&layout.data)?;
    Ok(())
}

/// Old certificates remain selected until approval for the replacement succeeds.
pub fn trust_update_needed(layout: &Layout) -> bool {
    let Ok(bytes) = policy::read_certificate(&root_certificate(layout)) else {
        return false;
    };
    policy::validate(&bytes, false, true).is_err()
        || state_directory(layout).join("retired-pending").exists()
}

pub fn ensure_trust(layout: &Layout) -> Result<()> {
    let _lock = MaterialLock::acquire(layout)?;
    ensure_trust_with(layout, install_trust)
}
fn ensure_trust_with(layout: &Layout, install: impl Fn(&Layout) -> Result<()>) -> Result<()> {
    ensure_certificate_inner(layout)?;
    if policy::validate(
        &policy::read_certificate(&root_certificate(layout))?,
        false,
        true,
    )
    .is_err()
    {
        let staged = Layout::at(
            layout.home.clone(),
            state_directory(layout).join("upgrade"),
            layout.runtime.clone(),
        );
        fs::create_dir_all(&staged.data).map_err(|e| e.to_string())?;
        staged.save_projects(&layout.registered_projects()?)?;
        ensure_certificate_inner(&staged)?;
        crate::durability::sync_tree(&staged.data)?;
        install(&staged)?;
        // Publish one pointer rather than overwriting a root/key/leaf quartet.
        // If reload fails or the app exits, the retired root is still trusted.
        atomic_write(&state_directory(layout).join("retired-pending"), b"pending")?;
        crate::durability::sync_directory(&state_directory(layout))?;
        atomic_write(&state_directory(layout).join("active-v2"), b"2")?;
        atomic_write(&trust_marker(layout), b"trusted")?;
        crate::durability::sync_directory(&state_directory(layout))?;
        return Ok(());
    }
    install(layout)
}

/// Called only after the new Nginx configuration has been applied successfully.
pub fn finish_trust_update(layout: &Layout) -> Result<()> {
    let _lock = MaterialLock::acquire(layout)?;
    finish_trust_update_with(layout, remove_certificate_trust)
}
fn finish_trust_update_with(
    layout: &Layout,
    remove: impl Fn(&Layout, &std::path::Path) -> Result<()>,
) -> Result<()> {
    let pending = state_directory(layout).join("retired-pending");
    if !pending.exists() || !state_directory(layout).join("active-v2").exists() {
        return Ok(());
    }
    let retired = state_directory(layout).join("root-ca.pem");
    remove(layout, &retired)?;
    fs::remove_file(pending).map_err(|e| e.to_string())?;
    crate::durability::sync_directory(&state_directory(layout))
}

fn run(mut command: Command) -> Result<()> {
    let result = command.output().map_err(|e| e.to_string())?;
    if result.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!(
                "Command exited with {}: {}",
                result.status,
                String::from_utf8_lossy(&result.stdout).trim()
            )
        } else {
            detail
        })
    }
}

#[cfg(target_os = "macos")]
fn macos_keychain(layout: &Layout) -> PathBuf {
    layout.home.join("Library/Keychains/login.keychain-db")
}

#[cfg(target_os = "macos")]
fn verify_macos_trust(layout: &Layout) -> Result<()> {
    let mut verify = Command::new("/usr/bin/security");
    verify
        .args(["verify-cert", "-c"])
        .arg(certificate(layout))
        .args(["-p", "ssl", "-n", "fairway.test", "-k"])
        .arg(macos_keychain(layout));
    run(verify)
}

fn install_trust(layout: &Layout) -> Result<()> {
    ensure_certificate_inner(layout)?;
    policy::validate(
        &policy::read_certificate(&root_certificate(layout))?,
        false,
        true,
    )?;
    #[cfg(not(target_os = "macos"))]
    let root = root_certificate(layout);
    #[cfg(target_os = "macos")]
    {
        if verify_macos_trust(layout).is_ok() {
            atomic_write(&trust_marker(layout), b"trusted")?;
            return Ok(());
        }
        let keychain = macos_keychain(layout);
        if !keychain.is_file() {
            return Err(
                "Your macOS login keychain is unavailable. Open Keychain Access and try again."
                    .into(),
            );
        }
        // Native Security calls and the compatibility security subprocess can
        // both ask for keychain/trust approval. Cover either implementation.
        #[cfg(feature = "desktop")]
        let _authorization = crate::desktop::authorization::begin()?;
        #[cfg(feature = "native-macos-trust")]
        macos_native::install(&root_certificate(layout), &keychain)?;
        #[cfg(not(feature = "native-macos-trust"))]
        {
            let mut install = Command::new("/usr/bin/security");
            install
                .args(["add-trusted-cert", "-r", "trustRoot", "-p", "ssl", "-k"])
                .arg(keychain)
                .arg(root_certificate(layout));
            run(install)
                .map_err(|e| format!("Could not trust Fairway's local certificate: {e}"))?;
        }
        verify_macos_trust(layout).map_err(|e| {
            format!("macOS did not trust Fairway's local certificate after installation: {e}")
        })?;
    }
    #[cfg(windows)]
    {
        let mut install = crate::process::command("certutil.exe");
        install.args(["-user", "-addstore", "Root"]).arg(root);
        run(install).map_err(|e| format!("Could not trust Fairway's local certificate: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::IsTerminal;
        let program = if std::io::stdin().is_terminal() {
            "sudo"
        } else {
            "pkexec"
        };
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut install = Command::new(program);
        install.arg(executable).arg("__trust-ca").arg(root);
        run(install).map_err(|e| format!("Could not trust Fairway's local certificate: {e}"))?;
    }
    atomic_write(&trust_marker(layout), b"trusted")?;
    Ok(())
}

pub fn remove_trust(layout: &Layout) -> Result<()> {
    let _lock = MaterialLock::acquire(layout)?;
    let root = root_certificate(layout);
    if !trust_marker(layout).exists() {
        return clear_https(layout);
    }
    if !root.exists() {
        return Err(
            "The local CA certificate is missing; certificate trust could not be removed.".into(),
        );
    }
    remove_certificate_trust(layout, &root)?;
    if state_directory(layout).join("retired-pending").exists() {
        remove_certificate_trust(layout, &state_directory(layout).join("root-ca.pem"))?;
        fs::remove_file(state_directory(layout).join("retired-pending"))
            .map_err(|e| e.to_string())?;
    }
    fs::remove_file(trust_marker(layout)).map_err(|e| e.to_string())?;
    clear_https(layout)
}

#[cfg(any(
    target_os = "linux",
    windows,
    not(feature = "native-macos-trust"),
    test
))]
fn certificate_hash(der: &[u8], legacy: bool) -> String {
    use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY, SHA256};
    digest(
        if legacy {
            &SHA1_FOR_LEGACY_USE_ONLY
        } else {
            &SHA256
        },
        der,
    )
    .as_ref()
    .iter()
    .map(|byte| format!("{byte:02X}"))
    .collect()
}
fn remove_certificate_trust(layout: &Layout, root: &std::path::Path) -> Result<()> {
    #[cfg(not(target_os = "macos"))]
    let _ = layout;
    policy::validate(&policy::read_certificate(root)?, true, false)?;
    #[cfg(target_os = "macos")]
    {
        // Trust removal can prompt just like installation. The native call
        // presents that prompt in-process, which is the path whose Touch ID
        // dialog already receives focus. Hold the handoff across the cleanup.
        let keychain = macos_keychain(layout);
        if !keychain.is_file() {
            return Err(
                "Your macOS login keychain is unavailable. Open Keychain Access and try again."
                    .into(),
            );
        }
        #[cfg(feature = "desktop")]
        let _authorization = crate::desktop::authorization::begin()?;
        #[cfg(feature = "native-macos-trust")]
        macos_native::remove_certificate(root, &keychain)?;
        #[cfg(not(feature = "native-macos-trust"))]
        {
            let der = policy::validate(&policy::read_certificate(root)?, true, false)?;
            let hash = certificate_hash(&der, true);
            let mut find = Command::new("/usr/bin/security");
            let found = find
                .args(["find-certificate", "-a", "-Z", "-c", CA_NAME])
                .arg(&keychain)
                .output()
                .map_err(|e| e.to_string())?;
            if !found.status.success() && found.status.code() != Some(44) {
                return Err(String::from_utf8_lossy(&found.stderr).into());
            }
            if !String::from_utf8_lossy(&found.stdout).lines().any(|line| {
                line.strip_prefix("SHA-1 hash: ")
                    .is_some_and(|saved| saved.trim() == hash)
            }) {
                return Ok(());
            }
            let mut delete = Command::new("/usr/bin/security");
            delete
                .args(["delete-certificate", "-t", "-Z", &hash])
                .arg(&keychain);
            run(delete)
                .map_err(|e| format!("Could not remove Fairway's local certificate trust: {e}"))?;
        }
    }
    #[cfg(windows)]
    {
        let der = policy::validate(&policy::read_certificate(root)?, true, false)?;
        let thumbprint = certificate_hash(&der, true);
        let mut remove = crate::process::command("certutil.exe");
        remove.args(["-user", "-delstore", "Root", &thumbprint]);
        run(remove)
            .map_err(|e| format!("Could not remove Fairway's local certificate trust: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::IsTerminal;
        let program = if std::io::stdin().is_terminal() {
            "sudo"
        } else {
            "pkexec"
        };
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut remove = Command::new(program);
        remove.arg(executable).arg("__remove-ca").arg(root);
        run(remove)
            .map_err(|e| format!("Could not remove Fairway's local certificate trust: {e}"))?;
    }
    Ok(())
}

#[cfg(any(target_os = "linux", all(test, unix)))]
pub fn apply_linux_trust(root: &std::path::Path, remove: bool) -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("Administrator permission is required to trust the local certificate.".into());
    }
    // Read once, without following links or blocking on special files. Every
    // store operation below uses these validated bytes, never the supplied path.
    let bytes = policy::read_certificate(root)?;
    let der = policy::validate(&bytes, remove, !remove)?;
    let destination = std::path::Path::new("/usr/local/share/ca-certificates");
    use std::os::unix::fs::MetadataExt;
    if destination.is_dir() && std::path::Path::new("/usr/sbin/update-ca-certificates").is_file() {
        let parent = fs::metadata(destination).map_err(|e| e.to_string())?;
        if parent.uid() != 0 || parent.mode() & 0o022 != 0 {
            return Err("The system CA directory must be owned and writable only by root.".into());
        }
        update_linux_anchor(destination, &bytes, &der, remove)?;
        let mut update = Command::new("/usr/sbin/update-ca-certificates");
        update
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LANG", "C");
        return run(update);
    }
    struct Temporary(std::path::PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let path = Temporary(
        std::env::temp_dir().join(format!("fairway-ca-{:032x}.pem", rand::random::<u128>())),
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path.0)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    let mut command = Command::new("/usr/bin/trust");
    command
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LANG", "C");
    command
        .arg("anchor")
        .arg(if remove { "--remove" } else { "--store" })
        .arg(&path.0);
    run(command)
}

#[cfg(any(target_os = "linux", test))]
fn update_linux_anchor(
    destination: &std::path::Path,
    bytes: &[u8],
    der: &[u8],
    remove: bool,
) -> Result<()> {
    let named = destination.join(format!("fairway-xmp-{}.crt", certificate_hash(der, false)));
    if !remove {
        atomic_write(&named, bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&named, fs::Permissions::from_mode(0o644))
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    for path in [named, destination.join("fairway-xmp-local.crt")] {
        if !path.exists() {
            continue;
        }
        let existing = policy::read_certificate(&path)?;
        // The legacy fixed filename may belong to another profile. Never remove
        // it on the strength of its name alone.
        if policy::validate(&existing, true, false).is_ok_and(|saved| saved == der) {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_trust_clears_saved_https_choices_without_a_trust_marker() {
        let root = std::env::temp_dir().join(format!("fairway-tls-test-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(&layout.data).unwrap();
        let project = crate::config::ProjectConfig {
            name: "Hello".into(),
            domain: "hello".into(),
            root: root.clone(),
            https: true,
            index_fallback: false,
        };
        layout.save_projects(&[project]).unwrap();
        remove_trust(&layout).unwrap();
        assert!(!layout.registered_projects().unwrap()[0].https);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
#[path = "tls/boundary_tests.rs"]
mod boundary_tests;
