//! Platform credential stores. No command-line tools, password files or fallback.
use super::*;
use std::sync::Arc;
#[cfg(any(target_os = "linux", all(test, unix)))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(windows, test))]
mod windows;
pub(super) trait Vault: Send + Sync {
    fn create(&self, id: &str, password: &Password) -> Result<()>;
    fn read(&self, id: &str) -> Result<Password>;
    fn remove(&self, id: &str) -> Result<()>;
}
pub(super) fn system() -> Arc<dyn Vault> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(macos::Keychain)
    }
    #[cfg(windows)]
    {
        Arc::new(windows::CredentialManager)
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(linux::SecretService)
    }
}
fn validate_id(id: &str) -> Result<()> {
    if identity(id) {
        Ok(())
    } else {
        Err(fail(
            Code::InvalidRequest,
            "Invalid saved credential identity",
        ))
    }
}
fn validate_password(password: &Password) -> Result<()> {
    if password.bytes().len() <= 4096 && !password.bytes().contains(&0) {
        Ok(())
    } else {
        Err(fail(Code::InvalidRequest, "Invalid password"))
    }
}
