use super::*;
use crate::config::ProjectConfig;
#[cfg(unix)]
use std::path::Path;

struct Fixture {
    layout: Layout,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("fairway-boundary-{:032x}", rand::random::<u128>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(&layout.data).unwrap();
        layout
            .save_projects(&[ProjectConfig {
                name: "Shop".into(),
                domain: "shop".into(),
                root: root.join("shop"),
                https: true,
                index_fallback: false,
            }])
            .unwrap();
        Self { layout }
    }
    fn legacy(&self) -> Vec<u8> {
        let dir = directory(&self.layout);
        fs::create_dir_all(&dir).unwrap();
        let key = KeyPair::generate().unwrap();
        let ca = CertifiedIssuer::self_signed(old_params(), &key).unwrap();
        fs::write(root_key(&self.layout), key.serialize_pem()).unwrap();
        fs::write(root_certificate(&self.layout), ca.pem()).unwrap();
        fs::write(trust_marker(&self.layout), b"trusted").unwrap();
        ensure_certificate(&self.layout).unwrap();
        ca.pem().into_bytes()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.layout.data.parent().unwrap());
    }
}
fn old_params() -> CertificateParams {
    let mut params = ca_params();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .name_constraints
        .as_mut()
        .unwrap()
        .excluded_subtrees
        .clear();
    params.extended_key_usages.clear();
    params
}
fn pem(params: CertificateParams) -> Vec<u8> {
    CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap())
        .unwrap()
        .pem()
        .into_bytes()
}
#[test]
fn generated_roots_and_keys_match_the_enforced_profile() {
    let fixture = Fixture::new();
    ensure_certificate(&fixture.layout).unwrap();
    let bytes = policy::read_certificate(&root_certificate(&fixture.layout)).unwrap();
    policy::validate(&bytes, false, true).unwrap();
    let key = KeyPair::from_pem(&fs::read_to_string(root_key(&fixture.layout)).unwrap()).unwrap();
    policy::key_matches(&bytes, &key).unwrap();
    assert!(policy::key_matches(&bytes, &KeyPair::generate().unwrap()).is_err());
    let original = fs::read(certificate(&fixture.layout)).unwrap();
    ensure_certificate(&fixture.layout).unwrap();
    assert_eq!(fs::read(certificate(&fixture.layout)).unwrap(), original);
}
#[test]
fn privileged_install_rejects_broad_dns_ip_usage_subordinate_and_non_ca_certificates() {
    let mut bad = Vec::new();
    bad.push(CertificateParams::default());
    bad.push(old_params());
    let mut params = ca_params();
    params.name_constraints = None;
    bad.push(params);
    let mut params = ca_params();
    params
        .name_constraints
        .as_mut()
        .unwrap()
        .permitted_subtrees
        .push(GeneralSubtree::DnsName(".com".into()));
    bad.push(params);
    let mut params = ca_params();
    params
        .name_constraints
        .as_mut()
        .unwrap()
        .excluded_subtrees
        .pop();
    bad.push(params);
    let mut params = ca_params();
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(1));
    bad.push(params);
    let mut params = ca_params();
    params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::CodeSigning);
    bad.push(params);
    let mut params = ca_params();
    params.not_after = OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_before -= time::Duration::days(20);
    bad.push(params);
    let mut params = ca_params();
    params
        .distinguished_name
        .push(DnType::CommonName, "Another CA");
    bad.push(params);
    for params in bad {
        assert!(policy::validate(&pem(params), false, true).is_err());
    }
    assert!(policy::validate(&pem(old_params()), true, false).is_ok());
}
#[test]
fn certificate_parser_rejects_bundles_extra_content_invalid_signatures_and_private_keys() {
    let good = pem(ca_params());
    assert!(policy::validate(&[good.clone(), good.clone()].concat(), false, true).is_err());
    assert!(policy::validate(&[b"garbage".to_vec(), good.clone()].concat(), false, true).is_err());
    assert!(policy::validate(&[good.clone(), b"garbage".to_vec()].concat(), false, true).is_err());
    assert!(policy::validate(
        KeyPair::generate().unwrap().serialize_pem().as_bytes(),
        false,
        true
    )
    .is_err());
    let mut der = policy::validate(&good, false, true).unwrap();
    let last = der.len() - 1;
    der[last] ^= 1;
    use base64::Engine;
    let corrupt = format!(
        "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
        base64::engine::general_purpose::STANDARD.encode(der)
    );
    assert!(policy::validate(corrupt.as_bytes(), false, true).is_err());
}
#[test]
fn migration_cancellation_and_cleanup_retry_preserve_enabled_sites_and_retired_material() {
    let fixture = Fixture::new();
    let layout = &fixture.layout;
    let old = fixture.legacy();
    let old_leaf = fs::read(certificate(layout)).unwrap();
    assert!(trust_update_needed(layout));
    assert!(ensure_trust_with(layout, |staged| {
        assert_eq!(
            policy::read_certificate(&root_certificate(layout)).unwrap(),
            old
        );
        policy::validate(
            &policy::read_certificate(&root_certificate(staged))?,
            false,
            true,
        )?;
        Err("Approval cancelled".into())
    })
    .unwrap_err()
    .contains("cancelled"));
    assert_eq!(fs::read(certificate(layout)).unwrap(), old_leaf);
    assert!(!state_directory(layout).join("active-v2").exists());
    ensure_trust_with(layout, |_| Ok(())).unwrap();
    assert_ne!(
        policy::read_certificate(&root_certificate(layout)).unwrap(),
        old
    );
    assert_eq!(
        fs::read(state_directory(layout).join("root-ca.pem")).unwrap(),
        old
    );
    assert!(layout.registered_projects().unwrap()[0].https);
    assert!(trust_update_needed(layout));
    assert!(finish_trust_update_with(layout, |_, root| {
        assert_eq!(fs::read(root).unwrap(), old);
        Err("Removal cancelled".into())
    })
    .is_err());
    assert!(trust_update_needed(layout));
    ensure_trust_with(layout, |current| {
        assert_eq!(root_certificate(current), root_certificate(layout));
        Ok(())
    })
    .unwrap();
    finish_trust_update_with(layout, |_, root| {
        assert_eq!(fs::read(root).unwrap(), old);
        Ok(())
    })
    .unwrap();
    assert!(!trust_update_needed(layout));
    assert!(layout.registered_projects().unwrap()[0].https);
}
#[test]
fn linux_anchors_coexist_and_removal_is_by_certificate_contents() {
    let fixture = Fixture::new();
    let store = fixture.layout.data.join("store");
    fs::create_dir(&store).unwrap();
    let old = pem(old_params());
    let new = pem(ca_params());
    let old_der = policy::validate(&old, true, false).unwrap();
    let new_der = policy::validate(&new, false, true).unwrap();
    fs::write(store.join("fairway-xmp-local.crt"), &old).unwrap();
    update_linux_anchor(&store, &new, &new_der, false).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let installed = store.join(format!(
            "fairway-xmp-{}.crt",
            certificate_hash(&new_der, false)
        ));
        assert_eq!(
            fs::metadata(installed).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }
    assert_eq!(fs::read_dir(&store).unwrap().count(), 2);
    // Removing a different profile's root must retain the old shared filename.
    update_linux_anchor(&store, &new, &new_der, true).unwrap();
    assert_eq!(fs::read(store.join("fairway-xmp-local.crt")).unwrap(), old);
    update_linux_anchor(&store, &new, &new_der, false).unwrap();
    update_linux_anchor(&store, &old, &old_der, true).unwrap();
    assert_eq!(fs::read_dir(&store).unwrap().count(), 1);
    update_linux_anchor(&store, &old, &old_der, true).unwrap();
    assert_eq!(fs::read_dir(&store).unwrap().count(), 1);
}
#[cfg(unix)]
#[test]
fn privileged_reads_refuse_links_special_files_and_oversized_inputs() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let root = fixture.layout.data.join("root-ca.pem");
    fs::write(&root, pem(ca_params())).unwrap();
    let link = fixture.layout.data.join("alias");
    symlink(&root, &link).unwrap();
    assert!(policy::read_certificate(&link).is_err());
    assert!(policy::read_certificate(Path::new("/dev/zero")).is_err());
    fs::write(&root, vec![b'x'; 32769]).unwrap();
    assert!(policy::read_certificate(&root).is_err());
}
#[cfg(target_os = "macos")]
#[test]
fn actual_chain_validation_allows_test_dns_and_checks_ip_and_subordinate_constraints() {
    let fixture = Fixture::new();
    let dir = &fixture.layout.data;
    let key = KeyPair::generate().unwrap();
    let ca = CertifiedIssuer::self_signed(ca_params(), &key).unwrap();
    let root = dir.join("root.pem");
    fs::write(&root, ca.pem()).unwrap();
    let verify = |file: &Path, intermediate: Option<&Path>| {
        let mut cmd = Command::new(
            std::env::var_os("FAIRWAY_TEST_OPENSSL").unwrap_or_else(|| "openssl".into()),
        );
        cmd.args(["verify", "-purpose", "sslserver", "-CAfile"])
            .arg(&root);
        if let Some(path) = intermediate {
            cmd.arg("-untrusted").arg(path);
        }
        cmd.arg(file).output().unwrap().status.success()
    };
    for (name, allowed) in [
        ("shop.test", true),
        ("example.com", false),
        ("127.0.0.1", false),
        ("10.20.30.40", false),
        ("::1", false),
    ] {
        let mut leaf = CertificateParams::new(vec![name.into()]).unwrap();
        leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let cert = leaf.signed_by(&KeyPair::generate().unwrap(), &ca).unwrap();
        let file = dir.join("leaf.pem");
        fs::write(&file, cert.pem()).unwrap();
        assert_eq!(verify(&file, None), allowed, "{name}");
    }
    let mut intermediate = ca_params();
    intermediate
        .distinguished_name
        .push(DnType::CommonName, "Subordinate CA");
    let intermediate =
        CertifiedIssuer::signed_by(intermediate, KeyPair::generate().unwrap(), &ca).unwrap();
    let file = dir.join("intermediate.pem");
    fs::write(&file, intermediate.pem()).unwrap();
    let leaf = CertificateParams::new(vec!["shop.test".into()])
        .unwrap()
        .signed_by(&KeyPair::generate().unwrap(), &intermediate)
        .unwrap();
    let leaf_path = dir.join("leaf.pem");
    fs::write(&leaf_path, leaf.pem()).unwrap();
    let accepted = verify(&leaf_path, Some(&file));
    let version =
        Command::new(std::env::var_os("FAIRWAY_TEST_OPENSSL").unwrap_or_else(|| "openssl".into()))
            .arg("version")
            .output()
            .unwrap();
    if String::from_utf8_lossy(&version.stdout).starts_with("OpenSSL ") {
        assert!(
            !accepted,
            "OpenSSL accepted a subordinate under a pathLen=0 root"
        );
    } else {
        eprintln!("Platform verifier root path-length acceptance: {accepted}; see docs/security-boundaries.md. DNS/IP rejection was verified separately.");
    }
}

#[test]
fn cached_leaf_is_repaired_when_its_key_or_issuer_changes() {
    let fixture = Fixture::new();
    let layout = &fixture.layout;
    ensure_certificate(layout).unwrap();
    let root = policy::read_certificate(&root_certificate(layout)).unwrap();
    fs::write(
        private_key(layout),
        KeyPair::generate().unwrap().serialize_pem(),
    )
    .unwrap();
    ensure_certificate(layout).unwrap();
    let domains = vec!["fairway.test".into(), "shop.test".into()];
    assert!(policy::leaf_matches(
        &fs::read(certificate(layout)).unwrap(),
        &fs::read_to_string(private_key(layout)).unwrap(),
        &root,
        &domains
    ));
    assert_eq!(fs::read(root_certificate(layout)).unwrap(), root);
    let replacement_key = KeyPair::generate().unwrap();
    let replacement = CertifiedIssuer::self_signed(ca_params(), &replacement_key).unwrap();
    fs::write(root_certificate(layout), replacement.pem()).unwrap();
    fs::write(root_key(layout), replacement_key.serialize_pem()).unwrap();
    ensure_certificate(layout).unwrap();
    assert!(policy::leaf_matches(
        &fs::read(certificate(layout)).unwrap(),
        &fs::read_to_string(private_key(layout)).unwrap(),
        replacement.pem().as_bytes(),
        &domains
    ));
}

#[test]
fn concurrent_material_operations_do_not_replace_keys_during_approval() {
    let fixture = Fixture::new();
    let layout = &fixture.layout;
    ensure_certificate(layout).unwrap();
    let before = fs::read(root_key(layout)).unwrap();
    let lock = MaterialLock::acquire(layout).unwrap();
    assert!(ensure_certificate(layout)
        .unwrap_err()
        .contains("in progress"));
    assert_eq!(fs::read(root_key(layout)).unwrap(), before);
    drop(lock);
    ensure_certificate(layout).unwrap();
}
