//! Opt-in native trust experiment. Keep the login keychain and user/SSL scope.
use crate::config::Result;
use core_foundation::{
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use security_framework::{
    base::Error,
    certificate::SecCertificate,
    item::{
        AddRef, ItemAddOptions, ItemAddValue, ItemClass, ItemSearchOptions, Limit, Location,
        Reference, SearchResult,
    },
    os::macos::{certificate::SecCertificateExt, keychain::SecKeychain},
    policy::SecPolicy,
    secure_transport::SslProtocolSide,
};
use security_framework_sys::{
    base::{
        errSecDuplicateItem, errSecItemNotFound, errSecNoTrustSettings, errSecSuccess,
        SecCertificateRef,
    },
    trust_settings::{
        kSecTrustSettingsDomainUser, kSecTrustSettingsResultTrustRoot, SecTrustSettingsDomain,
        SecTrustSettingsSetTrustSettings,
    },
};
use std::{fs, path::Path};

const USER_CANCELLED: i32 = -128;
const AUTHORIZATION_CANCELLED: i32 = -60006;

extern "C" {
    fn SecTrustSettingsRemoveTrustSettings(
        certificate: SecCertificateRef,
        domain: SecTrustSettingsDomain,
    ) -> i32;
}

fn trust_setting_keys() -> (CFString, CFString) {
    // SecTrustSettings.h defines these keys as CFSTR macros, not exported symbols.
    (
        CFString::from_static_string("kSecTrustSettingsPolicy"),
        CFString::from_static_string("kSecTrustSettingsResult"),
    )
}

fn certificate_from_pem(bytes: &[u8]) -> Result<SecCertificate> {
    let pem =
        pem::parse(bytes).map_err(|e| format!("Cannot read the local CA certificate: {e}"))?;
    if pem.tag() != "CERTIFICATE" {
        return Err("The local CA file is not a certificate. Its contents were preserved.".into());
    }
    let certificate = SecCertificate::from_der(pem.contents())
        .map_err(|e| format!("Cannot decode the local CA certificate: {e}"))?;
    if certificate.subject_summary() != super::CA_NAME {
        return Err(
            "The local CA certificate has an unexpected name. Its contents were preserved.".into(),
        );
    }
    Ok(certificate)
}

fn ssl_root_settings() -> CFDictionary<CFString, CFType> {
    let policy = SecPolicy::create_ssl(SslProtocolSide::SERVER, None);
    let (policy_key, result_key) = trust_setting_keys();
    CFDictionary::from_CFType_pairs(&[
        (policy_key, policy.as_CFType()),
        (
            result_key,
            CFNumber::from(kSecTrustSettingsResultTrustRoot as i32).as_CFType(),
        ),
    ])
}

fn native_error(stage: &str, error: Error) -> String {
    if error.code() == USER_CANCELLED || error.code() == AUTHORIZATION_CANCELLED {
        format!("Certificate setup was cancelled while {stage}. You can retry HTTPS setup.")
    } else {
        format!("Could not {stage}: {error} (macOS error {}).", error.code())
    }
}

pub(super) fn install(root: &Path, keychain: &Path) -> Result<()> {
    // Decode the actual PEM used by verification, not a potentially stale DER sidecar.
    let certificate = certificate_from_pem(&fs::read(root).map_err(|e| e.to_string())?)?;
    let keychain =
        SecKeychain::open(keychain).map_err(|e| native_error("open your login keychain", e))?;
    let mut add = ItemAddOptions::new(ItemAddValue::Ref(AddRef::Certificate(certificate.clone())));
    add.set_location(Location::FileKeychain(keychain));
    if let Err(error) = add.add() {
        // A prior cancelled trust approval may have already inserted this public certificate.
        if error.code() != errSecDuplicateItem {
            return Err(native_error(
                "add the local CA to your login keychain",
                error,
            ));
        }
    }
    let settings = ssl_root_settings();
    // SAFETY: the certificate and retained CF dictionary remain alive for the call.
    // Only SSL server trust in the current user's domain is requested. This may
    // display Apple's authorization UI; the caller already runs off the UI thread.
    let status = unsafe {
        SecTrustSettingsSetTrustSettings(
            certificate.as_concrete_TypeRef(),
            kSecTrustSettingsDomainUser,
            settings.as_CFTypeRef(),
        )
    };
    if status != errSecSuccess {
        return Err(native_error(
            "approve SSL trust for the local CA",
            Error::from_code(status),
        ));
    }
    Ok(())
}

fn cancelled(error: Error) -> bool {
    error.code() == USER_CANCELLED || error.code() == AUTHORIZATION_CANCELLED
}

fn removal_error(stage: &str, error: Error) -> String {
    if cancelled(error) {
        format!("Certificate removal was cancelled while {stage}. You can retry.")
    } else {
        format!("Could not {stage}: {error} (macOS error {}).", error.code())
    }
}

/// A certificate belongs to Fairway when either its common name or its
/// subject summary is the local CA name. `security find-certificate -c` matches
/// the common name; older copies use the same name.
pub(super) fn is_fairway_ca(common_name: Option<&str>, summary: &str) -> bool {
    common_name == Some(super::CA_NAME) || summary == super::CA_NAME
}

pub(super) fn trust_settings_absent(status: i32) -> bool {
    status == errSecNoTrustSettings || status == errSecItemNotFound
}

fn fairway_certificates(keychain: &SecKeychain) -> Result<Vec<SecCertificate>> {
    let found = ItemSearchOptions::new()
        .keychains(std::slice::from_ref(keychain))
        .class(ItemClass::certificate())
        .load_refs(true)
        .limit(Limit::All)
        .search();
    let found = match found {
        Ok(items) => items,
        Err(error) if error.code() == errSecItemNotFound => return Ok(Vec::new()),
        Err(error) => return Err(removal_error("find Fairway's local certificate", error)),
    };
    let mut certificates = Vec::new();
    for item in found {
        let SearchResult::Ref(Reference::Certificate(certificate)) = item else {
            continue;
        };
        let summary = certificate.subject_summary();
        let common_name = certificate.common_name().ok();
        if is_fairway_ca(common_name.as_deref(), &summary) {
            certificates.push(certificate);
        }
    }
    Ok(certificates)
}

pub(super) fn remove_certificate(root: &Path, keychain_path: &Path) -> Result<()> {
    let target = certificate_from_pem(&super::policy::read_certificate(root)?)?.to_der();
    let keychain = SecKeychain::open(keychain_path)
        .map_err(|error| removal_error("open your login keychain", error))?;
    for certificate in fairway_certificates(&keychain)? {
        if certificate.to_der() != target {
            continue;
        }
        // SAFETY: the certificate remains alive for the call. Only the current
        // user's trust domain is changed. This may display Apple's authorization
        // UI; the caller already relinquished focus and runs off the UI thread.
        let status = unsafe {
            SecTrustSettingsRemoveTrustSettings(
                certificate.as_concrete_TypeRef(),
                kSecTrustSettingsDomainUser,
            )
        };
        if status != errSecSuccess && !trust_settings_absent(status) {
            return Err(removal_error(
                "approve removal of SSL trust for the local CA",
                Error::from_code(status),
            ));
        }
        // A public certificate left after a cancelled trust change must remain
        // until this call succeeds, so a retry can find it again.
        if let Err(error) = certificate.delete() {
            if error.code() != errSecItemNotFound {
                return Err(removal_error(
                    "remove the local CA from your login keychain",
                    error,
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertifiedIssuer, KeyPair};

    #[test]
    fn native_trust_uses_the_supplied_ca_and_rejects_other_material() {
        let key = KeyPair::generate().unwrap();
        let ca = CertifiedIssuer::self_signed(super::super::ca_params(), &key).unwrap();
        let parsed = certificate_from_pem(ca.pem().as_bytes()).unwrap();
        assert_eq!(parsed.to_der(), ca.der().as_ref());
        assert!(certificate_from_pem(key.serialize_pem().as_bytes()).is_err());
        assert!(certificate_from_pem(b"not a certificate").is_err());
        let other =
            CertifiedIssuer::self_signed(rcgen::CertificateParams::default(), &key).unwrap();
        assert!(certificate_from_pem(other.pem().as_bytes()).is_err());
    }

    #[test]
    fn native_trust_has_explicit_ssl_policy_and_root_result() {
        let settings = ssl_root_settings();
        let (policy_key, result_key) = trust_setting_keys();
        assert_eq!(settings.len(), 2);
        assert!(settings.get(policy_key).downcast::<SecPolicy>().is_some());
        let result = settings.get(result_key).downcast::<CFNumber>().unwrap();
        assert_eq!(
            result.to_i32(),
            Some(kSecTrustSettingsResultTrustRoot as i32)
        );
    }

    #[test]
    fn native_errors_preserve_cancellation_and_failure_context() {
        assert!(
            native_error("approving trust", Error::from_code(USER_CANCELLED)).contains("cancelled")
        );
        assert!(
            native_error("approving trust", Error::from_code(AUTHORIZATION_CANCELLED))
                .contains("cancelled")
        );
        let error = native_error("open your login keychain", Error::from_code(-25293));
        assert!(error.contains("open your login keychain") && error.contains("-25293"));
    }

    #[test]
    fn removal_selects_only_the_fairway_ca_and_preserves_cancellation() {
        let name = super::super::CA_NAME;
        assert!(is_fairway_ca(Some(name), name));
        assert!(is_fairway_ca(Some(name), "other"));
        assert!(is_fairway_ca(None, name));
        assert!(!is_fairway_ca(Some("Another CA"), "Another CA"));
        assert!(!is_fairway_ca(None, ""));
        assert!(trust_settings_absent(errSecNoTrustSettings));
        assert!(trust_settings_absent(errSecItemNotFound));
        assert!(!trust_settings_absent(errSecSuccess));
        let cancelled = removal_error(
            "approve removal of SSL trust for the local CA",
            Error::from_code(USER_CANCELLED),
        );
        assert!(cancelled.contains("cancelled") && cancelled.contains("retry"));
        let failed = removal_error("open your login keychain", Error::from_code(-25293));
        assert!(failed.contains("open your login keychain") && failed.contains("-25293"));
    }
}
