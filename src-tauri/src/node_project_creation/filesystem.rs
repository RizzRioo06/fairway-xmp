//! macOS descriptor-relative IO. Never follow links or replace a publication target.
use super::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::{Component, Path},
};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Identity {
    pub dev: u64,
    pub ino: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Receipt {
    pub identity: Identity,
    pub sha256: String,
    pub bytes: u64,
    pub mode: u32,
    pub modified: (i64, i64),
}
pub(super) fn failure(error: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorCode::FilesystemFailed,
        format!("Local generation IO failed; uncertain data was preserved: {error}"),
    )
}
fn c(name: &str) -> Result<CString> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return Err(Error::new(
            ErrorCode::UnsafeTemplate,
            "Unsafe descriptor-relative name.",
        ));
    }
    CString::new(name).map_err(failure)
}
fn checked(value: i32) -> Result<i32> {
    if value < 0 {
        Err(failure(std::io::Error::last_os_error()))
    } else {
        Ok(value)
    }
}
fn identity(metadata: &std::fs::Metadata) -> Identity {
    Identity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    }
}
pub(super) struct Dir(File);
impl Dir {
    pub fn absolute(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(failure("An absolute anchored path is required."));
        }
        let mut current = Self(File::open("/").map_err(failure)?);
        for part in path.components() {
            match part {
                Component::RootDir => (),
                Component::Normal(name) => {
                    current =
                        current.child(name.to_str().ok_or_else(|| failure("Non-UTF-8 folder"))?)?;
                }
                _ => return Err(failure("Unsafe absolute path component")),
            }
        }
        Ok(current)
    }
    pub fn id(&self) -> Result<Identity> {
        Ok(identity(&self.0.metadata().map_err(failure)?))
    }
    pub fn identity_text(&self) -> Result<String> {
        let id = self.id()?;
        Ok(format!("{}:{}", id.dev, id.ino))
    }
    pub fn assert_path(&self, path: &Path) -> Result<()> {
        if Self::absolute(path)?.id()? != self.id()? {
            return Err(Error::new(
                ErrorCode::RecoveryUnsafe,
                "The anchored directory path was replaced. Data was preserved.",
            ));
        }
        Ok(())
    }
    pub fn child(&self, name: &str) -> Result<Self> {
        let fd = checked(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                c(name)?.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })?;
        Ok(Self(unsafe { File::from_raw_fd(fd) }))
    }
    pub fn private(&self) -> Result<()> {
        let metadata = self.0.metadata().map_err(failure)?;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o7777 != 0o700 {
            return Err(Error::new(
                ErrorCode::RecoveryUnsafe,
                "Job directories must be private and owned by this account.",
            ));
        }
        Ok(())
    }
    pub fn make(&self, name: &str) -> Result<Self> {
        checked(unsafe { libc::mkdirat(self.0.as_raw_fd(), c(name)?.as_ptr(), 0o700) })?;
        let dir = self.child(name)?;
        dir.private()?;
        self.sync()?;
        Ok(dir)
    }
    pub fn entry(&self, name: &str) -> Result<Option<(Identity, u32)>> {
        let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
        if unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                c(name)?.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } < 0
        {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(failure(error));
        }
        let stat = unsafe { stat.assume_init() };
        Ok(Some((
            Identity {
                dev: stat.st_dev as u64,
                ino: stat.st_ino,
            },
            u32::from(stat.st_mode),
        )))
    }
    pub fn verify_child(&self, name: &str, dir: &Self) -> Result<()> {
        if self
            .entry(name)?
            .map(|(id, mode)| (id, mode & libc::S_IFMT as u32))
            != Some((dir.id()?, libc::S_IFDIR as u32))
        {
            return Err(Error::new(
                ErrorCode::RecoveryUnsafe,
                "The job directory entry was replaced. Data was preserved.",
            ));
        }
        Ok(())
    }
    pub fn names(&self) -> Result<Vec<String>> {
        let fd = checked(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        })?;
        let raw = unsafe { libc::fdopendir(fd) };
        if raw.is_null() {
            unsafe { libc::close(fd) };
            return Err(failure(std::io::Error::last_os_error()));
        }
        struct Listing(*mut libc::DIR);
        impl Drop for Listing {
            fn drop(&mut self) {
                unsafe { libc::closedir(self.0) };
            }
        }
        let listing = Listing(raw);
        let mut names = Vec::new();
        loop {
            unsafe {
                *libc::__error() = 0;
            }
            let entry = unsafe { libc::readdir(listing.0) };
            if entry.is_null() {
                if std::io::Error::last_os_error().raw_os_error() != Some(0) {
                    return Err(failure(std::io::Error::last_os_error()));
                }
                break;
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_str()
                .map_err(failure)?;
            if name == "." || name == ".." {
                continue;
            }
            c(name)?;
            names.push(name.to_owned());
            if names.len() > 512 {
                return Err(failure("Too many job directory entries"));
            }
        }
        names.sort();
        Ok(names)
    }
    pub fn create(&self, name: &str, bytes: &[u8]) -> Result<Receipt> {
        let fd = checked(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                c(name)?.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        })?;
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(bytes).map_err(failure)?;
        file.sync_all().map_err(failure)?;
        let metadata = file.metadata().map_err(failure)?;
        self.sync()?;
        Ok(Receipt {
            identity: identity(&metadata),
            sha256: super::digest(bytes),
            bytes: bytes.len() as u64,
            mode: metadata.mode() & 0o7777,
            modified: (metadata.mtime(), metadata.mtime_nsec()),
        })
    }
    pub fn read(&self, name: &str, limit: u64) -> Result<(Vec<u8>, Receipt)> {
        let fd = checked(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                c(name)?.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        })?;
        let file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata().map_err(failure)?;
        if !metadata.is_file()
            || metadata.len() > limit
            || metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err(failure(
                "Expected an owned, bounded regular file with one link",
            ));
        }
        let mut bytes = Vec::new();
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(failure)?;
        if bytes.len() as u64 > limit {
            return Err(failure("File grew beyond its bound"));
        }
        Ok((
            bytes.clone(),
            Receipt {
                identity: identity(&metadata),
                sha256: super::digest(&bytes),
                bytes: bytes.len() as u64,
                mode: metadata.mode() & 0o7777,
                modified: (metadata.mtime(), metadata.mtime_nsec()),
            },
        ))
    }
    pub fn atomic(&self, name: &str, bytes: &[u8]) -> Result<()> {
        if let Some((_, mode)) = self.entry(name)? {
            if mode & libc::S_IFMT as u32 != libc::S_IFREG as u32 {
                return Err(failure("Journal target is not a regular file"));
            }
        }
        let temporary = format!(".journal-{:032x}.partial", rand::random::<u128>());
        self.create(&temporary, bytes)?;
        let result = checked(unsafe {
            libc::renameat(
                self.0.as_raw_fd(),
                c(&temporary)?.as_ptr(),
                self.0.as_raw_fd(),
                c(name)?.as_ptr(),
            )
        })
        .map(|_| ());
        if result.is_err() {
            let _ = self.unlink_file(&temporary);
        }
        result?;
        self.sync()
    }
    pub fn no_replace(&self, from: &str, to: &str) -> Result<()> {
        let result = unsafe {
            libc::renameatx_np(
                self.0.as_raw_fd(),
                c(from)?.as_ptr(),
                self.0.as_raw_fd(),
                c(to)?.as_ptr(),
                libc::RENAME_EXCL,
            )
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(Error::new(
                    ErrorCode::DestinationExists,
                    "Atomic publication refused an existing destination.",
                ));
            }
            return Err(failure(error));
        }
        self.sync()
    }
    pub fn unlink_file(&self, name: &str) -> Result<()> {
        checked(unsafe { libc::unlinkat(self.0.as_raw_fd(), c(name)?.as_ptr(), 0) }).map(|_| ())?;
        self.sync()
    }
    pub fn unlink_dir(&self, name: &str) -> Result<()> {
        checked(unsafe {
            libc::unlinkat(self.0.as_raw_fd(), c(name)?.as_ptr(), libc::AT_REMOVEDIR)
        })
        .map(|_| ())?;
        self.sync()
    }
    pub fn sync(&self) -> Result<()> {
        self.0.sync_all().map_err(failure)
    }
    pub fn lock(&self, name: &str) -> Result<File> {
        let fd = checked(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                c(name)?.as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC,
                0o600,
            )
        })?;
        let file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata().map_err(failure)?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o7777 != 0o600
        {
            return Err(failure("Unsafe creation lock file"));
        }
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(Error::new(
                ErrorCode::GenerationBusy,
                "Another creation/recovery operation owns the journal lock.",
            ));
        }
        Ok(file)
    }
}
pub(super) fn directories(files: impl Iterator<Item = String>) -> Vec<String> {
    let mut dirs = std::collections::BTreeSet::new();
    for path in files {
        let parts: Vec<_> = path.split('/').collect();
        for end in 1..parts.len() {
            dirs.insert(parts[..end].join("/"));
        }
    }
    let mut dirs: Vec<_> = dirs.into_iter().collect();
    dirs.sort_by_key(|name| (name.split('/').count(), name.clone()));
    dirs
}
pub(super) fn parent_of(
    root: &Dir,
    path: &str,
    dirs: &BTreeMap<String, Identity>,
) -> Result<(Dir, String)> {
    super::assets::validate_entry(path)?;
    let parts: Vec<_> = path.split('/').collect();
    let mut dir = root.child_internal()?;
    let mut relative = String::new();
    for part in &parts[..parts.len() - 1] {
        if !relative.is_empty() {
            relative.push('/');
        }
        relative.push_str(part);
        let expected = dirs
            .get(&relative)
            .ok_or_else(|| failure("Unjournaled parent directory"))?;
        dir = dir.child(part)?;
        if &dir.id()? != expected {
            return Err(Error::new(
                ErrorCode::RecoveryUnsafe,
                "A nested staging directory was replaced; no writes were redirected.",
            ));
        }
        dir.private()?;
    }
    Ok((dir, parts.last().unwrap().to_string()))
}
impl Dir {
    fn child_internal(&self) -> Result<Self> {
        let fd = checked(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        })?;
        Ok(Self(unsafe { File::from_raw_fd(fd) }))
    }
}
/// Match the complete tree, including inode receipts and unexpected entries.
pub(super) fn verify_tree(
    root: &Dir,
    files: &BTreeMap<String, Receipt>,
    dirs: &BTreeMap<String, Identity>,
) -> Result<()> {
    fn walk(
        root: &Dir,
        prefix: &str,
        found: &mut Vec<String>,
        dirs: &BTreeMap<String, Identity>,
    ) -> Result<()> {
        for name in root.names()? {
            let relative = format!("{prefix}{name}");
            let (_, mode) = root
                .entry(&name)?
                .ok_or_else(|| failure("Entry disappeared"))?;
            found.push(relative.clone());
            if found.len() > 256 || relative.len() > 1024 {
                return Err(failure("Staging tree is too large"));
            }
            if mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32 {
                let child = root.child(&name)?;
                if dirs.get(&relative) != Some(&child.id()?) {
                    return Err(Error::new(
                        ErrorCode::RecoveryUnsafe,
                        "An unknown/replaced nested directory was preserved.",
                    ));
                }
                walk(&child, &format!("{relative}/"), found, dirs)?;
            } else if mode & libc::S_IFMT as u32 != libc::S_IFREG as u32 {
                return Err(Error::new(
                    ErrorCode::RecoveryUnsafe,
                    "Linked/special staging entries were preserved.",
                ));
            }
            if found.len() > 256 {
                return Err(failure("Staging tree is too large"));
            }
        }
        Ok(())
    }
    root.private()?;
    let mut found = Vec::new();
    walk(root, "", &mut found, dirs)?;
    found.sort();
    let mut expected: Vec<_> = files.keys().chain(dirs.keys()).cloned().collect();
    expected.sort();
    if found != expected {
        return Err(Error::new(
            ErrorCode::RecoveryUnsafe,
            "Unknown or unjournaled staging entries were preserved.",
        ));
    }
    for (name, id) in dirs {
        let (parent, leaf) = parent_of(root, name, dirs)?;
        let dir = parent.child(&leaf)?;
        dir.private()?;
        if &dir.id()? != id {
            return Err(failure("Staging directory identity changed"));
        }
    }
    for (name, receipt) in files {
        let (parent, leaf) = parent_of(root, name, dirs)?;
        if parent.read(&leaf, 256 * 1024)?.1 != *receipt {
            return Err(Error::new(
                ErrorCode::RecoveryUnsafe,
                "Modified staging files were preserved.",
            ));
        }
    }
    Ok(())
}
pub(super) fn clean(
    root: &Dir,
    files: &BTreeMap<String, Receipt>,
    dirs: &BTreeMap<String, Identity>,
) -> Result<()> {
    verify_tree(root, files, dirs)?;
    for (name, receipt) in files {
        let (parent, leaf) = parent_of(root, name, dirs)?;
        if parent.read(&leaf, 256 * 1024)?.1 != *receipt {
            return Err(failure("File changed during cleanup"));
        }
        parent.unlink_file(&leaf)?;
    }
    let mut names: Vec<_> = dirs.keys().collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.split('/').count()));
    for name in names {
        let (parent, leaf) = parent_of(root, name, dirs)?;
        if parent.child(&leaf)?.id()? != dirs[name] {
            return Err(failure("Directory changed during cleanup"));
        }
        parent.unlink_dir(&leaf)?;
    }
    Ok(())
}
