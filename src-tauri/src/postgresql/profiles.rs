//! Explicit external connection profiles. Metadata never contains passwords.
use super::explorer_contract::*;
use crate::config::Layout;
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod files;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod native;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod vault;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    pub remember_password: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Save {
    pub confirmed: bool,
    pub profile: Profile,
    /// None keeps an existing vault item; Some replaces it (including empty passwords).
    pub password: Option<Password>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Remove {
    pub confirmed: bool,
    pub id: String,
    pub revision: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Open {
    pub confirmed: bool,
    pub id: String,
    pub revision: String,
    pub password: Option<Password>,
}
pub struct Profiles {
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    native: native::Store,
}
impl Profiles {
    pub fn new(layout: Option<Layout>) -> Self {
        #[cfg(any(target_os = "macos", target_os = "linux", windows))]
        {
            Self {
                native: native::Store::new(layout.map(|l| l.data)),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            let _ = layout;
            Self {}
        }
    }
    pub fn list(&self) -> Result<Vec<Profile>> {
        #[cfg(any(target_os = "macos", target_os = "linux", windows))]
        {
            self.native.list()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            Err(fail(
                Code::Unavailable,
                "Saved connections are unavailable on this operating system",
            ))
        }
    }
    pub fn save(&self, request: Save) -> Result<Profile> {
        #[cfg(any(target_os = "macos", target_os = "linux", windows))]
        {
            self.native.save(request)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            let _ = request;
            Err(fail(
                Code::Unavailable,
                "Saved connections are unavailable on this operating system",
            ))
        }
    }
    pub fn remove(&self, request: Remove) -> Result<()> {
        #[cfg(any(target_os = "macos", target_os = "linux", windows))]
        {
            self.native.remove(request)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            let _ = request;
            Err(fail(
                Code::Unavailable,
                "Saved connections are unavailable on this operating system",
            ))
        }
    }
    pub fn connection(&self, request: Open) -> Result<Connect> {
        #[cfg(any(target_os = "macos", target_os = "linux", windows))]
        {
            self.native.connection(request)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        {
            let _ = request;
            Err(fail(
                Code::Unavailable,
                "Saved connections are unavailable on this operating system",
            ))
        }
    }
}
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn identity(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn validate(p: &Profile) -> Result<()> {
    if !identity(&p.id)
        || (!p.revision.is_empty() && !identity(&p.revision))
        || p.name.trim().is_empty()
        || p.name.len() > 160
        || p.name.chars().any(char::is_control)
        || !["127.0.0.1", "::1"].contains(&p.host.as_str())
        || p.port == 0
    {
        return Err(fail(
            Code::InvalidRequest,
            "Check the connection name, host and port",
        ));
    }
    for value in [&p.user, &p.database] {
        if value.is_empty() || value.len() > 63 || value.contains('\0') {
            return Err(fail(
                Code::InvalidRequest,
                "Database and user must contain 1–63 bytes without NUL",
            ));
        }
    }
    Ok(())
}
