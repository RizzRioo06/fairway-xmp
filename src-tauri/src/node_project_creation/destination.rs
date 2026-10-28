//! Read-only validation. It does not reserve a destination or test it by writing.
use super::{Error, ErrorCode, Result};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Destination {
    pub parent: PathBuf,
    pub root: PathBuf,
    pub identity: String,
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidDestination, message)
}
pub(super) fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_lowercase()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || name.ends_with('-')
        || matches!(
            name,
            "con"
                | "prn"
                | "aux"
                | "nul"
                | "com1"
                | "com2"
                | "com3"
                | "com4"
                | "com5"
                | "com6"
                | "com7"
                | "com8"
                | "com9"
                | "lpt1"
                | "lpt2"
                | "lpt3"
                | "lpt4"
                | "lpt5"
                | "lpt6"
                | "lpt7"
                | "lpt8"
                | "lpt9"
                | "node-modules"
        )
    {
        return Err(Error::new(ErrorCode::InvalidName, "Use 1–64 lowercase ASCII letters, digits or hyphens, starting with a letter and ending with a letter/digit; avoid reserved device names."));
    }
    Ok(())
}
pub(super) fn validate(parent: &str, name: &str) -> Result<Destination> {
    validate_name(name)?;
    let path = Path::new(parent);
    if !path.is_absolute()
        || parent.len() > 8192
        || parent.contains(':')
        || parent.chars().any(char::is_control)
        || path
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid("Choose an existing absolute parent folder without traversal, control characters or PATH delimiters."));
    }
    let parent = fs::canonicalize(path)
        .map_err(|_| invalid("The destination parent folder is unavailable."))?;
    let text = parent
        .to_str()
        .ok_or_else(|| invalid("The parent path must be UTF-8."))?;
    if text.len() + name.len() + 1 > 8192
        || text.contains(':')
        || text.chars().any(char::is_control)
    {
        return Err(invalid("The canonical destination path is unsupported."));
    }
    let metadata =
        fs::metadata(&parent).map_err(|_| invalid("The parent folder is unavailable."))?;
    if !metadata.is_dir() {
        return Err(invalid("Choose a parent folder, not a file."));
    }
    #[cfg(unix)]
    let identity = {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::MetadataExt;
        let path = std::ffi::CString::new(parent.as_os_str().as_bytes())
            .map_err(|_| invalid("Invalid parent path."))?;
        if metadata.mode() & 0o222 == 0
            || metadata.mode() & 0o111 == 0
            || unsafe { libc::access(path.as_ptr(), libc::W_OK | libc::X_OK) } != 0
        {
            return Err(invalid(
                "The parent folder must be writable and searchable.",
            ));
        }
        format!("{}:{}", metadata.dev(), metadata.ino())
    };
    #[cfg(not(unix))]
    let identity = format!("{:?}:{:?}", parent, metadata.created());
    let root = parent.join(name);
    match fs::symlink_metadata(&root) {
        Ok(_) => return Err(Error::new(ErrorCode::DestinationExists, "The destination already exists, including files, empty folders and symbolic links. Choose another name.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => return Err(invalid("The destination could not be checked safely.")),
    }
    Ok(Destination {
        parent,
        root,
        identity,
    })
}
