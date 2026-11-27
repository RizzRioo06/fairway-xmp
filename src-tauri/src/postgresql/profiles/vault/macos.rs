//! Separate Keychain namespace; never reads or changes managed cluster credentials.
use crate::postgresql::explorer_contract::*;
fn unavailable(message: &str) -> Error {
    fail(Code::Unavailable, message)
}
pub struct Keychain;
impl Keychain {
    fn query(
        id: &str,
    ) -> core_foundation::dictionary::CFMutableDictionary<
        core_foundation::base::CFType,
        core_foundation::base::CFType,
    > {
        use core_foundation::{base::TCFType, string::CFString};
        use security_framework_sys::item::*;
        unsafe extern "C" {
            static kSecUseAuthenticationUIFail: core_foundation::string::CFStringRef;
        }
        unsafe {
            core_foundation::dictionary::CFMutableDictionary::from_CFType_pairs(&[
                (
                    CFString::wrap_under_get_rule(kSecClass).as_CFType(),
                    CFString::wrap_under_get_rule(kSecClassGenericPassword).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrService).as_CFType(),
                    CFString::new("dev.fairway.external-postgresql.v1").as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrAccount).as_CFType(),
                    CFString::new(id).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecUseAuthenticationUI).as_CFType(),
                    CFString::wrap_under_get_rule(kSecUseAuthenticationUIFail).as_CFType(),
                ),
            ])
        }
    }
}
impl super::Vault for Keychain {
    fn create(&self, id: &str, secret: &Password) -> Result<()> {
        super::validate_id(id)?;
        super::validate_password(secret)?;
        use core_foundation::{base::TCFType, data::CFData, string::CFString};
        use security_framework_sys::{item::kSecValueData, keychain_item::SecItemAdd};
        let mut query = Self::query(id);
        unsafe {
            query.set(
                CFString::wrap_under_get_rule(kSecValueData).as_CFType(),
                CFData::from_buffer(secret.bytes()).as_CFType(),
            );
        }
        let code = unsafe { SecItemAdd(query.as_concrete_TypeRef(), std::ptr::null_mut()) };
        if code != 0 {
            return Err(unavailable(
                "Could not save password in Keychain. No plaintext fallback was used",
            ));
        }
        Ok(())
    }
    fn read(&self, id: &str) -> Result<Password> {
        super::validate_id(id)?;
        use core_foundation::{base::TCFType, boolean::CFBoolean, data::CFData, string::CFString};
        use security_framework_sys::{item::kSecReturnData, keychain_item::SecItemCopyMatching};
        let mut query = Self::query(id);
        unsafe {
            query.set(
                CFString::wrap_under_get_rule(kSecReturnData).as_CFType(),
                CFBoolean::true_value().as_CFType(),
            );
        }
        let mut result = std::ptr::null();
        let code = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };
        if code != 0 || result.is_null() {
            return Err(unavailable(
                "Saved password is unavailable. Enter it again or update the connection",
            ));
        }
        let data = unsafe { CFData::wrap_under_create_rule(result.cast()) };
        Password::from_bytes(data.bytes())
    }
    fn remove(&self, id: &str) -> Result<()> {
        super::validate_id(id)?;
        use core_foundation::base::TCFType;
        use security_framework_sys::keychain_item::SecItemDelete;
        let code = unsafe { SecItemDelete(Self::query(id).as_concrete_TypeRef()) };
        if code == 0 || code == -25300 {
            Ok(())
        } else {
            Err(unavailable("Connection metadata was updated, but Keychain cleanup is pending. Retry the action after unlocking Keychain"))
        }
    }
}
