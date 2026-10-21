//! Validate the certificate bytes before any privileged trust-store operation.
use crate::config::Result;
use rcgen::PublicKeyData;
use std::{fs::OpenOptions, io::Read, path::Path};
use x509_parser::{extensions::GeneralName, prelude::*};

pub(super) fn read_certificate(path: &Path) -> Result<Vec<u8>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 32768 {
        return Err("The local CA must be one certificate of at most 32 KiB.".into());
    }
    let mut bytes = Vec::new();
    file.take(32769)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32768 {
        return Err("The local CA file is too large.".into());
    }
    Ok(bytes)
}

pub(super) fn validate(bytes: &[u8], legacy: bool, valid_now: bool) -> Result<Vec<u8>> {
    let invalid = || {
        "The local CA does not match Fairway's restricted HTTPS certificate policy. Its files were preserved.".to_string()
    };
    if bytes.len() > 32768 {
        return Err(invalid());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?.trim();
    if !text.starts_with("-----BEGIN CERTIFICATE-----") {
        return Err(invalid());
    }
    let (rest, pem) = parse_x509_pem(text.as_bytes()).map_err(|_| invalid())?;
    if pem.label != "CERTIFICATE" || !rest.iter().all(u8::is_ascii_whitespace) {
        return Err(invalid());
    }
    let (rest, cert) = X509Certificate::from_der(&pem.contents).map_err(|_| invalid())?;
    if !rest.is_empty()
        || cert.subject() != cert.issuer()
        || cert.subject().iter_attributes().count() != 1
        || cert
            .subject()
            .iter_common_name()
            .next()
            .and_then(|cn| cn.as_str().ok())
            != Some(super::CA_NAME)
        || (valid_now && !cert.validity().is_valid())
    {
        return Err(invalid());
    }
    cert.verify_signature(None).map_err(|_| invalid())?;
    cert.extensions_map().map_err(|_| invalid())?;
    for extension in cert.extensions() {
        if ![
            "2.5.29.19",
            "2.5.29.15",
            "2.5.29.30",
            "2.5.29.37",
            "2.5.29.14",
            "2.5.29.35",
        ]
        .contains(&extension.oid.to_id_string().as_str())
        {
            return Err(invalid());
        }
    }
    let bc = cert
        .basic_constraints()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let usage = cert
        .key_usage()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let nc = cert
        .name_constraints()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    if !bc.critical || !bc.value.ca || !usage.critical || usage.value.flags != 96 || !nc.critical {
        return Err(invalid());
    }
    let permitted = nc.value.permitted_subtrees.as_deref().ok_or_else(invalid)?;
    if permitted.len() != 1 || !matches!(permitted[0].base, GeneralName::DNSName(".test")) {
        return Err(invalid());
    }
    let excluded = nc.value.excluded_subtrees.as_deref().unwrap_or_default();
    let restricted_ips = excluded.len() == 2 && [8, 32].iter().all(|length| excluded.iter().any(|item| matches!(&item.base, GeneralName::IPAddress(bytes) if bytes.len() == *length && bytes.iter().all(|b| *b == 0))));
    let eku = cert.extended_key_usage().map_err(|_| invalid())?;
    let restricted_usage = eku.as_ref().is_some_and(|ext| {
        let e = ext.value;
        e.server_auth
            && !e.any
            && !e.client_auth
            && !e.code_signing
            && !e.email_protection
            && !e.time_stamping
            && !e.ocsp_signing
            && e.other.is_empty()
    });
    let strict = bc.value.path_len_constraint == Some(0) && restricted_ips && restricted_usage;
    let old = bc.value.path_len_constraint.is_none() && excluded.is_empty() && eku.is_none();
    if !strict && !(legacy && old) {
        return Err(invalid());
    }
    Ok(pem.contents)
}

pub(super) fn key_matches(bytes: &[u8], key: &rcgen::KeyPair) -> Result<()> {
    let der = validate(bytes, true, false)?;
    let (_, certificate) = X509Certificate::from_der(&der).map_err(|e| e.to_string())?;
    if certificate.public_key().raw != key.subject_public_key_info() {
        return Err(
            "The local CA certificate and private key do not match. Their files were preserved."
                .into(),
        );
    }
    Ok(())
}

pub(super) fn leaf_matches(cert: &[u8], key: &str, root: &[u8], domains: &[String]) -> bool {
    let check = || -> Option<()> {
        let (_, pem) = parse_x509_pem(cert).ok()?;
        let (_, leaf) = X509Certificate::from_der(&pem.contents).ok()?;
        let key = rcgen::KeyPair::from_pem(key).ok()?;
        let der = validate(root, true, false).ok()?;
        let (_, issuer) = X509Certificate::from_der(&der).ok()?;
        if !leaf.validity().is_valid()
            || leaf.issuer() != issuer.subject()
            || leaf.public_key().raw != key.subject_public_key_info()
        {
            return None;
        }
        leaf.verify_signature(Some(issuer.public_key())).ok()?;
        let san = leaf.subject_alternative_name().ok()??;
        let names: Option<Vec<String>> = san
            .value
            .general_names
            .iter()
            .map(|name| match name {
                GeneralName::DNSName(name) => Some(name.to_string()),
                _ => None,
            })
            .collect();
        if names?.as_slice() != domains {
            return None;
        }
        Some(())
    };
    check().is_some()
}
