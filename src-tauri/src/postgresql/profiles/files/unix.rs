//! Unix profile metadata: descriptor-relative paths, ownership, links, locks and durable commits.
use crate::postgresql::{Error, ErrorCode};
type Result<T> = std::result::Result<T, Error>;
fn failure(message: &str) -> Error {
    Error::new(ErrorCode::Unavailable, message)
}
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::{Component, Path},
};
#[derive(PartialEq, Eq)]
pub struct Identity {
    dev: u64,
    ino: u64,
}
pub struct Dir(File);
fn name(value: &str) -> Result<CString> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains(['/', '\\'])
        || value.chars().any(char::is_control)
    {
        return Err(failure("Unsafe managed path"));
    }
    CString::new(value).map_err(|_| failure("Unsafe managed path"))
}
fn owned(meta: &std::fs::Metadata) -> bool {
    meta.uid() == unsafe { libc::geteuid() }
}
fn fd(value: i32) -> Result<File> {
    if value < 0 {
        Err(failure("Managed filesystem operation failed"))
    } else {
        Ok(unsafe { File::from_raw_fd(value) })
    }
}
impl Dir {
    pub fn absolute(path: &Path) -> Result<Self> {
        if !path.is_absolute() {
            return Err(failure("Expected a canonical absolute managed path"));
        }
        let mut dir = Self(File::open("/").map_err(|_| failure("Cannot open filesystem root"))?);
        for part in path.components() {
            match part {
                Component::RootDir => (),
                Component::Normal(part) => {
                    dir = dir.child(
                        part.to_str()
                            .ok_or_else(|| failure("Non-UTF8 managed path"))?,
                    )?
                }
                _ => return Err(failure("Unsafe managed path")),
            }
        }
        Ok(dir)
    }
    pub fn identity(&self) -> Result<Identity> {
        let m = self
            .0
            .metadata()
            .map_err(|_| failure("Missing directory metadata"))?;
        Ok(Identity {
            dev: m.dev(),
            ino: m.ino(),
        })
    }
    pub fn private(&self) -> Result<()> {
        let m = self
            .0
            .metadata()
            .map_err(|_| failure("Missing private directory"))?;
        if !m.is_dir() || !owned(&m) || m.mode() & 0o7777 != 0o700 {
            return Err(failure(
                "Managed directories must be owned, private mode 0700",
            ));
        }
        Ok(())
    }
    pub fn assert_path(&self, path: &Path) -> Result<()> {
        if Self::absolute(path)?.identity()? != self.identity()? {
            return Err(failure("Managed directory identity changed"));
        }
        Ok(())
    }
    pub fn child(&self, value: &str) -> Result<Self> {
        let n = name(value)?;
        fd(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                n.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        })
        .map(Self)
    }
    pub fn make(&self, value: &str) -> Result<Self> {
        let n = name(value)?;
        if unsafe { libc::mkdirat(self.0.as_raw_fd(), n.as_ptr(), 0o700) } != 0 {
            return Err(failure(
                "Managed destination already exists or cannot be created; existing data kept",
            ));
        }
        self.sync()?;
        let child = self.child(value)?;
        child.private()?;
        Ok(child)
    }
    pub fn existing_or_make(&self, value: &str) -> Result<Self> {
        let child = if self.exists(value)? {
            self.child(value)?
        } else {
            self.make(value)?
        };
        child.private()?;
        Ok(child)
    }
    pub fn exists(&self, value: &str) -> Result<bool> {
        let n = name(value)?;
        let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
        let r = unsafe {
            libc::fstatat(
                self.0.as_raw_fd(),
                n.as_ptr(),
                st.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if r == 0 {
            Ok(true)
        } else if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
            Ok(false)
        } else {
            Err(failure("Cannot inspect managed entry"))
        }
    }
    pub fn read(&self, value: &str, limit: u64) -> Result<Vec<u8>> {
        self.read_checked(value, limit, false)
    }
    fn read_checked(&self, value: &str, limit: u64, bundle: bool) -> Result<Vec<u8>> {
        let n = name(value)?;
        let mut f = fd(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                n.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        })?;
        let m = f
            .metadata()
            .map_err(|_| failure("Cannot inspect managed file"))?;
        if !m.is_file()
            || m.nlink() != 1
            || (!owned(&m) && !(bundle && m.uid() == 0))
            || (bundle && m.mode() & 0o7022 != 0)
            || m.len() > limit
        {
            return Err(failure("Unsafe or oversized managed file"));
        }
        let mut b = Vec::new();
        (&mut f)
            .take(limit + 1)
            .read_to_end(&mut b)
            .map_err(|_| failure("Cannot read managed file"))?;
        let after = f
            .metadata()
            .map_err(|_| failure("Managed file disappeared"))?;
        if b.len() as u64 > limit
            || m.len() != after.len()
            || m.mtime() != after.mtime()
            || m.mtime_nsec() != after.mtime_nsec()
        {
            return Err(failure("Managed file changed while reading"));
        }
        Ok(b)
    }
    pub fn create(&self, value: &str, bytes: &[u8], executable: bool) -> Result<()> {
        let n = name(value)?;
        let mut f = fd(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                n.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                if executable { 0o500 } else { 0o600 },
            )
        })?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| failure("Cannot durably write managed file"))?;
        self.sync()
    }
    pub fn atomic(&self, value: &str, bytes: &[u8]) -> Result<()> {
        if self.exists(value)? {
            self.read(value, 1024 * 1024)?;
        }
        let temp = format!(".journal-{:032x}", rand::random::<u128>());
        self.create(&temp, bytes, false)?;
        if unsafe {
            libc::renameat(
                self.0.as_raw_fd(),
                name(&temp)?.as_ptr(),
                self.0.as_raw_fd(),
                name(value)?.as_ptr(),
            )
        } != 0
        {
            return Err(failure(
                "Cannot commit managed journal; private stage retained",
            ));
        }
        self.sync()
    }
    pub fn lock(&self, value: &str) -> Result<File> {
        self.lock_mode(value, libc::LOCK_EX)
    }
    fn lock_mode(&self, value: &str, mode: i32) -> Result<File> {
        let n = name(value)?;
        let f = fd(unsafe {
            libc::openat(
                self.0.as_raw_fd(),
                n.as_ptr(),
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC,
                0o600,
            )
        })?;
        let m = f
            .metadata()
            .map_err(|_| failure("Cannot inspect managed lock"))?;
        if !m.is_file()
            || !owned(&m)
            || m.nlink() != 1
            || m.mode() & 0o7777 != 0o600
            || unsafe { libc::flock(f.as_raw_fd(), mode | libc::LOCK_NB) } != 0
        {
            return Err(failure(
                "Managed runtime/cluster is leased or its lock is unsafe",
            ));
        }
        Ok(f)
    }
    pub fn sync(&self) -> Result<()> {
        self.0
            .sync_all()
            .map_err(|_| failure("Cannot sync managed directory"))
    }
}
