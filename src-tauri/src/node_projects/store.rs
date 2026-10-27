use super::{
    manifest, revision, runtime, validate_text, validate_url, Error, ErrorCode, ProjectConfig,
    Result,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const REGISTRY_LIMIT: u64 = 1024 * 1024;
const MAX_PROJECTS: usize = 64;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Document {
    pub schema_version: u32,
    pub projects: Vec<ProjectConfig>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            schema_version: 1,
            projects: Vec::new(),
        }
    }
}

fn validate(document: &Document) -> Result<()> {
    if document.schema_version != 1 || document.projects.len() > MAX_PROJECTS {
        return Err(Error::new(ErrorCode::RegistryUnavailable, "Unsupported Node registry version or more than 64 registrations. The saved file has been kept."));
    }
    let mut ids = HashSet::new();
    for (index, project) in document.projects.iter().enumerate() {
        if project.id.len() != 32
            || !project.id.bytes().all(|b| b.is_ascii_hexdigit())
            || !ids.insert(&project.id)
        {
            return Err(Error::new(
                ErrorCode::RegistryUnavailable,
                "The Node registry contains invalid or duplicate project IDs.",
            ));
        }
        validate_text(&project.name, 80, "Invalid saved project name.")?;
        validate_text(&project.script, 128, "Invalid saved package script.")?;
        validate_url(project.open_url.as_deref())?;
        for path in [&project.root, &project.runtime.node_path] {
            if !Path::new(path).is_absolute()
                || path.len() > 8192
                || path.chars().any(char::is_control)
            {
                return Err(Error::new(
                    ErrorCode::RegistryUnavailable,
                    "The Node registry contains an invalid absolute path.",
                ));
            }
        }
        if runtime::exact_version(&project.runtime.version).as_deref()
            != Some(&project.runtime.version)
            || document.projects[..index]
                .iter()
                .any(|other| manifest::same_root(&other.root, &project.root))
        {
            return Err(Error::new(ErrorCode::RegistryUnavailable, "The Node registry contains an invalid runtime version or duplicate project folder."));
        }
    }
    Ok(())
}

pub(super) fn load(data: &Path) -> Result<(Document, String)> {
    let bytes = manifest::read_regular(&data.join("node-projects.json"), REGISTRY_LIMIT).map_err(
        |error| {
            Error::new(
                ErrorCode::RegistryUnavailable,
                format!("Could not read node-projects.json; the file has been kept: {error}"),
            )
        },
    )?;
    let (document, bytes) = match bytes {
        Some(bytes) => {
            let document = serde_json::from_slice(&bytes).map_err(|error| {
                Error::new(
                    ErrorCode::RegistryUnavailable,
                    format!("node-projects.json is invalid; the file has been kept: {error}"),
                )
            })?;
            (document, bytes)
        }
        None => {
            let document = Document::default();
            let bytes = serde_json::to_vec_pretty(&document).expect("serialize empty registry");
            (document, bytes)
        }
    };
    validate(&document)
        .map_err(|error| Error::new(ErrorCode::RegistryUnavailable, error.message))?;
    Ok((document, revision(&bytes)))
}

pub(super) fn check_revision(expected: &str, actual: &str) -> Result<()> {
    if expected != actual {
        return Err(Error::new(
            ErrorCode::Conflict,
            "The registry or project inspection changed. Refresh before saving again.",
        ));
    }
    Ok(())
}

struct Staged(PathBuf);
impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub(super) fn save(data: &Path, expected: &str, document: &Document) -> Result<String> {
    save_with(data, expected, document, |_| Ok(()))
}

fn save_with(
    data: &Path,
    expected: &str,
    document: &Document,
    before_publish: impl FnOnce(&Path) -> Result<()>,
) -> Result<String> {
    validate(document)?;
    check_revision(expected, &load(data)?.1)?;
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| Error::new(ErrorCode::SaveFailed, error.to_string()))?;
    if bytes.len() as u64 > REGISTRY_LIMIT {
        return Err(Error::new(
            ErrorCode::SaveFailed,
            "The Node registry exceeds 1 MiB.",
        ));
    }
    let failed = |error| {
        Error::new(
            ErrorCode::SaveFailed,
            format!("The previous Node registry has been kept: {error}"),
        )
    };
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(data).map_err(failed)?;
    // Same-directory, unpredictable, exclusive staging avoids a shared temp
    // filename and preserves the prior complete document until publication.
    let staged = Staged(data.join(format!(
        ".node-projects-{:032x}.partial",
        rand::random::<u128>()
    )));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&staged.0).map_err(failed)?;
    file.write_all(&bytes).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    drop(file);
    before_publish(&staged.0)?;
    // Catch edits made during inspection/discovery or staging. Normal writes
    // are serialized by NodeProjects, independently from the PHP manager.
    check_revision(expected, &load(data)?.1)?;
    replace(&staged.0, &data.join("node-projects.json")).map_err(failed)?;
    crate::durability::sync_directory(data).map_err(|error| Error::new(ErrorCode::SaveFailed, format!("The Node registry was published, but directory syncing failed. Reload before retrying: {error}")))?;
    Ok(revision(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_publication_preserves_the_previous_file_and_retry_succeeds() {
        let data =
            std::env::temp_dir().join(format!("fw-node-store-{:032x}", rand::random::<u128>()));
        let document = Document::default();
        let initial = load(&data).unwrap().1;
        let saved = save(&data, &initial, &document).unwrap();
        let previous = fs::read(data.join("node-projects.json")).unwrap();
        let mut next = Document::default();
        next.projects.push(ProjectConfig {
            id: "a".repeat(32),
            name: "New registration".into(),
            root: data.join("project").to_str().unwrap().into(),
            script: "dev".into(),
            runtime: super::super::ProjectRuntime {
                node_path: data.join("node").to_str().unwrap().into(),
                version: "v22.1.0".into(),
            },
            open_url: None,
        });
        let error = save_with(&data, &saved, &next, |staged| {
            assert!(staged.is_file());
            Err(Error::new(
                ErrorCode::SaveFailed,
                "Simulated failure after file sync, before rename",
            ))
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::SaveFailed);
        assert_eq!(fs::read(data.join("node-projects.json")).unwrap(), previous);
        assert_eq!(fs::read_dir(&data).unwrap().count(), 1);
        assert_ne!(save(&data, &saved, &next).unwrap(), saved);
        assert_eq!(load(&data).unwrap().0.projects[0].name, "New registration");
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn edits_during_staging_are_preserved_and_rejected_as_conflicts() {
        let data =
            std::env::temp_dir().join(format!("fw-node-store-{:032x}", rand::random::<u128>()));
        let expected = load(&data).unwrap().1;
        let edited = br#"{"schemaVersion":1,"projects":[] }"#;
        let error = save_with(&data, &expected, &Document::default(), |_| {
            fs::write(data.join("node-projects.json"), edited).unwrap();
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::Conflict);
        assert_eq!(fs::read(data.join("node-projects.json")).unwrap(), edited);
        assert_eq!(fs::read_dir(&data).unwrap().count(), 1);
        fs::remove_dir_all(data).unwrap();
    }
}
