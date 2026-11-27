use super::files::Dir;
use super::vault::{self, Vault};
use super::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    profile: Profile,
    credential: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Journal {
    version: u32,
    profiles: Vec<Record>,
    retired: Vec<String>,
}
pub(super) struct Store {
    data: Option<PathBuf>,
    gate: Mutex<()>,
    vault: Arc<dyn Vault>,
}
fn io(_: crate::postgresql::Error) -> Error {
    fail(
        Code::Unavailable,
        "Saved connections are unavailable or unsafe; existing files were retained",
    )
}
fn encode(j: &Journal) -> Result<Vec<u8>> {
    serde_json::to_vec(j).map_err(|_| fail(Code::Unavailable, "Cannot encode saved connections"))
}
fn consent(value: bool) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(fail(
            Code::ConfirmationRequired,
            "Authorize this connection profile action",
        ))
    }
}
impl Store {
    pub fn new(data: Option<PathBuf>) -> Self {
        Self {
            data,
            gate: Mutex::new(()),
            vault: vault::system(),
        }
    }
    fn with<T>(&self, action: impl FnOnce(&Dir, &mut Journal) -> Result<T>) -> Result<T> {
        let _guard = self
            .gate
            .try_lock()
            .map_err(|_| fail(Code::Busy, "Saved connections are busy"))?;
        let path = self
            .data
            .as_ref()
            .ok_or_else(|| fail(Code::Unavailable, "Application data is unavailable"))?;
        let parent = Dir::absolute(path).map_err(io)?;
        let dir = parent
            .existing_or_make("postgresql-connections")
            .map_err(io)?;
        let _lease = dir.lock(".lock").map_err(io)?;
        dir.assert_path(&path.join("postgresql-connections"))
            .map_err(io)?;
        let mut journal = if dir.exists("profiles.json").map_err(io)? {
            let bytes = dir.read("profiles.json", 256 * 1024).map_err(io)?;
            serde_json::from_slice::<Journal>(&bytes).map_err(|_| {
                fail(
                    Code::Unavailable,
                    "Saved connections are damaged; existing data was retained",
                )
            })?
        } else {
            Journal {
                version: 1,
                profiles: vec![],
                retired: vec![],
            }
        };
        if journal.version != 1 || journal.profiles.len() > 64 || journal.retired.len() > 256 {
            return Err(fail(
                Code::Unavailable,
                "Unsupported saved connection metadata",
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for r in &journal.profiles {
            validate(&r.profile)?;
            if r.profile.revision.is_empty()
                || !seen.insert(&r.profile.id)
                || r.profile.remember_password != r.credential.is_some()
                || r.credential.as_ref().is_some_and(|c| !identity(c))
            {
                return Err(fail(Code::Unavailable, "Invalid saved connection metadata"));
            }
        }
        if journal.retired.iter().any(|c| {
            !identity(c)
                || journal
                    .profiles
                    .iter()
                    .any(|r| r.credential.as_ref() == Some(c))
        }) {
            return Err(fail(Code::Unavailable, "Invalid saved credential journal"));
        }
        action(&dir, &mut journal)
    }
    fn commit(dir: &Dir, j: &Journal) -> Result<()> {
        dir.atomic("profiles.json", &encode(j)?).map_err(io)
    }
    fn cleanup(&self, dir: &Dir, j: &mut Journal) -> Result<()> {
        // Intent stays durable until every deletion succeeds; retry after vault failure/crash.
        for id in &j.retired {
            self.vault.remove(id)?;
        }
        if !j.retired.is_empty() {
            j.retired.clear();
            Self::commit(dir, j)?;
        }
        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Profile>> {
        self.with(|_, j| Ok(j.profiles.iter().map(|r| r.profile.clone()).collect()))
    }
    pub fn save(&self, r: Save) -> Result<Profile> {
        consent(r.confirmed)?;
        validate(&r.profile)?;
        if let Some(p) = &r.password {
            if p.bytes().len() > 4096 || p.bytes().contains(&0) {
                return Err(fail(Code::InvalidRequest, "Invalid password"));
            }
        }
        self.with(|dir, j| {
            self.cleanup(dir, j)?;
            let index = j.profiles.iter().position(|p| p.profile.id == r.profile.id);
            let old = index.map(|i| &j.profiles[i]);
            if old.map(|p| p.profile.revision.as_str()).unwrap_or("") != r.profile.revision {
                return Err(fail(
                    Code::InvalidRequest,
                    "This connection changed. Refresh saved connections before editing",
                ));
            }
            if index.is_none() && j.profiles.len() >= 64 {
                return Err(fail(
                    Code::Busy,
                    "Remove a saved connection before adding another",
                ));
            }
            if r.profile.remember_password
                && r.password.is_none()
                && old.is_some_and(|old| {
                    old.profile.host != r.profile.host
                        || old.profile.port != r.profile.port
                        || old.profile.user != r.profile.user
                })
            {
                return Err(fail(
                    Code::InvalidRequest,
                    "Enter the password again when changing the server or user",
                ));
            }
            let old_secret = old.and_then(|r| r.credential.clone());
            let mut credential = if r.profile.remember_password {
                old_secret.clone()
            } else {
                None
            };
            if r.profile.remember_password {
                if let Some(password) = &r.password {
                    let id = format!("{:032x}", rand::random::<u128>());
                    // Persist cleanup intent before creating a vault item, including crash recovery.
                    j.retired.push(id.clone());
                    Self::commit(dir, j)?;
                    self.vault.create(&id, password)?;
                    credential = Some(id);
                } else if credential.is_none() {
                    return Err(fail(
                        Code::InvalidRequest,
                        "Enter a password to remember in secure storage",
                    ));
                }
            }
            let mut profile = r.profile;
            profile.name = profile.name.trim().to_owned();
            profile.revision = format!("{:032x}", rand::random::<u128>());
            if old_secret != credential {
                if let Some(id) = old_secret {
                    j.retired.push(id);
                }
            }
            if let Some(id) = &credential {
                j.retired.retain(|x| x != id);
            }
            let record = Record {
                profile: profile.clone(),
                credential,
            };
            if let Some(i) = index {
                j.profiles[i] = record;
            } else {
                j.profiles.push(record);
            }
            Self::commit(dir, j)?;
            // A cleanup failure is surfaced; committed profile remains readable and intent retries.
            self.cleanup(dir, j)?;
            Ok(profile)
        })
    }
    pub fn remove(&self, r: Remove) -> Result<()> {
        consent(r.confirmed)?;
        self.with(|dir, j| {
            self.cleanup(dir, j)?;
            let index = j
                .profiles
                .iter()
                .position(|p| p.profile.id == r.id && p.profile.revision == r.revision)
                .ok_or_else(|| {
                    fail(
                        Code::InvalidRequest,
                        "Connection changed or was removed. Refresh before retrying",
                    )
                })?;
            if let Some(id) = j.profiles.remove(index).credential {
                j.retired.push(id);
            }
            Self::commit(dir, j)?;
            self.cleanup(dir, j)
        })
    }
    pub fn connection(&self, r: Open) -> Result<Connect> {
        consent(r.confirmed)?;
        self.with(|_, j| {
            let record = j
                .profiles
                .iter()
                .find(|p| p.profile.id == r.id && p.profile.revision == r.revision)
                .ok_or_else(|| {
                    fail(
                        Code::InvalidRequest,
                        "Saved connection changed or no longer exists. Refresh and retry",
                    )
                })?;
            let p = &record.profile;
            let password = match r.password {
                Some(p) => p,
                None => match &record.credential {
                    Some(id) => self.vault.read(id)?,
                    None => {
                        return Err(fail(
                            Code::InvalidRequest,
                            "Enter this connection's password",
                        ))
                    }
                },
            };
            Ok(Connect {
                confirmed: true,
                database: p.database.clone(),
                target: Target::External {
                    host: p.host.clone(),
                    port: p.port,
                    user: p.user.clone(),
                    password,
                    transport: Transport::Loopback,
                },
            })
        })
    }
}
#[cfg(test)]
mod tests;
