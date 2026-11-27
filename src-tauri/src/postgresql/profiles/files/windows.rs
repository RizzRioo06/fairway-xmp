//! Private Windows profile metadata. Pinned ancestor handles refuse path replacement;
//! reparse points, hard links, foreign owners and permissive DACLs are rejected.
use crate::postgresql::{Error, ErrorCode};
use std::{
    ffi::c_void,
    path::{Component, Path, PathBuf},
    ptr,
    rc::Rc,
};
type Result<T> = std::result::Result<T, Error>;
type H = *mut c_void;
fn failure() -> Error {
    Error::new(
        ErrorCode::Unavailable,
        "Unsafe or unavailable saved connection storage; existing data was retained",
    )
}
#[repr(C)]
struct Security {
    size: u32,
    descriptor: H,
    inherit: i32,
}
#[repr(C)]
#[derive(Default, PartialEq, Eq)]
struct Info {
    attributes: u32,
    created: [u32; 2],
    accessed: [u32; 2],
    written: [u32; 2],
    volume: u32,
    high: u32,
    low: u32,
    links: u32,
    index_high: u32,
    index_low: u32,
}
#[repr(C)]
struct AclInfo {
    count: u32,
    used: u32,
    free: u32,
}
#[repr(C)]
struct Ace {
    kind: u8,
    flags: u8,
    size: u16,
    mask: u32,
    sid: u32,
}
#[link(name = "Kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        access: u32,
        share: u32,
        security: *const Security,
        disposition: u32,
        flags: u32,
        template: H,
    ) -> H;
    fn CreateDirectoryW(name: *const u16, security: *const Security) -> i32;
    fn CloseHandle(handle: H) -> i32;
    fn GetFileInformationByHandle(handle: H, info: *mut Info) -> i32;
    fn GetFileAttributesW(name: *const u16) -> u32;
    fn ReadFile(handle: H, buffer: *mut u8, count: u32, read: *mut u32, overlapped: H) -> i32;
    fn WriteFile(handle: H, buffer: *const u8, count: u32, written: *mut u32, overlapped: H)
        -> i32;
    fn FlushFileBuffers(handle: H) -> i32;
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    fn GetLastError() -> u32;
    fn GetCurrentProcess() -> H;
    fn LocalFree(value: H) -> H;
}
#[link(name = "Advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: H, access: u32, out: *mut H) -> i32;
    fn GetTokenInformation(token: H, class: u32, info: H, size: u32, needed: *mut u32) -> i32;
    fn ConvertSidToStringSidW(sid: H, out: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        value: *const u16,
        revision: u32,
        out: *mut H,
        size: *mut u32,
    ) -> i32;
    fn GetSecurityInfo(
        handle: H,
        kind: u32,
        flags: u32,
        owner: *mut H,
        group: *mut H,
        dacl: *mut H,
        sacl: *mut H,
        descriptor: *mut H,
    ) -> u32;
    fn GetSecurityDescriptorControl(value: H, control: *mut u16, revision: *mut u32) -> i32;
    fn EqualSid(a: H, b: H) -> i32;
    fn GetAclInformation(acl: H, info: H, size: u32, class: u32) -> i32;
    fn GetAce(acl: H, index: u32, out: *mut H) -> i32;
}
pub struct Handle(H);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
struct Local(H);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe { LocalFree(self.0) };
    }
}
fn wide(path: &Path) -> Result<Vec<u16>> {
    let value = path.to_str().ok_or_else(failure)?;
    if value.contains('\0') {
        return Err(failure());
    }
    Ok(value.encode_utf16().chain(Some(0)).collect())
}
fn current_sid() -> Result<(Vec<usize>, H)> {
    let mut token = ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), 8, &mut token) } == 0 {
        return Err(failure());
    }
    let token = Handle(token);
    let mut size = 0;
    unsafe { GetTokenInformation(token.0, 1, ptr::null_mut(), 0, &mut size) };
    if size == 0 || size > 65536 {
        return Err(failure());
    }
    // Pointer alignment is required by TOKEN_USER and SID_AND_ATTRIBUTES.
    let mut data = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe { GetTokenInformation(token.0, 1, data.as_mut_ptr().cast(), size, &mut size) } == 0 {
        return Err(failure());
    }
    let sid = unsafe { *(data.as_ptr().cast::<H>()) };
    if sid.is_null() {
        return Err(failure());
    }
    Ok((data, sid))
}
fn descriptor() -> Result<Local> {
    let (_data, sid) = current_sid()?;
    let mut string = ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut string) } == 0 || string.is_null() {
        return Err(failure());
    }
    let string = Local(string.cast());
    let mut length = 0;
    while unsafe { *(string.0.cast::<u16>().add(length)) } != 0 {
        length += 1;
        if length > 256 {
            return Err(failure());
        }
    }
    let sid =
        String::from_utf16(unsafe { std::slice::from_raw_parts(string.0.cast::<u16>(), length) })
            .map_err(|_| failure())?;
    let sddl: Vec<u16> = format!("O:{sid}D:P(A;;FA;;;{sid})")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut raw = ptr::null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut raw,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(failure());
    }
    Ok(Local(raw))
}
fn private(file: &Handle) -> Result<()> {
    let (_data, sid) = current_sid()?;
    let (mut owner, mut acl, mut raw) = (ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
    if unsafe {
        GetSecurityInfo(
            file.0,
            1,
            5,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut raw,
        )
    } != 0
    {
        return Err(failure());
    }
    let _descriptor = Local(raw);
    let (mut control, mut revision) = (0, 0);
    if owner.is_null()
        || acl.is_null()
        || unsafe { EqualSid(owner, sid) } == 0
        || unsafe { GetSecurityDescriptorControl(raw, &mut control, &mut revision) } == 0
        || control & 0x1000 == 0
    {
        return Err(failure());
    }
    let mut info = AclInfo {
        count: 0,
        used: 0,
        free: 0,
    };
    if unsafe {
        GetAclInformation(
            acl,
            (&mut info as *mut AclInfo).cast(),
            std::mem::size_of::<AclInfo>() as u32,
            2,
        )
    } == 0
        || info.count != 1
    {
        return Err(failure());
    }
    let mut entry = ptr::null_mut();
    if unsafe { GetAce(acl, 0, &mut entry) } == 0 || entry.is_null() {
        return Err(failure());
    }
    let ace = unsafe { &*entry.cast::<Ace>() };
    if ace.kind != 0
        || ace.flags != 0
        || ace.size < 12
        || ace.mask != 0x1f01ff
        || unsafe { EqualSid(ptr::addr_of!(ace.sid).cast_mut().cast(), sid) } == 0
    {
        return Err(failure());
    }
    Ok(())
}
fn info(file: &Handle, directory: bool) -> Result<Info> {
    let mut i = Info::default();
    if unsafe { GetFileInformationByHandle(file.0, &mut i) } == 0
        || i.attributes & 0x400 != 0
        || (i.attributes & 0x10 != 0) != directory
        || !directory && i.links != 1
    {
        return Err(failure());
    }
    Ok(i)
}
fn open(path: &Path, access: u32, share: u32, disposition: u32, directory: bool) -> Result<Handle> {
    let sd = descriptor()?;
    let security = Security {
        size: std::mem::size_of::<Security>() as u32,
        descriptor: sd.0,
        inherit: 0,
    };
    let file = unsafe {
        CreateFileW(
            wide(path)?.as_ptr(),
            access | 0x20000,
            share,
            &security,
            disposition,
            0x200000 | if directory { 0x2000000 } else { 0 },
            ptr::null_mut(),
        )
    };
    if file as isize == -1 {
        return Err(failure());
    }
    let file = Handle(file);
    info(&file, directory)?;
    Ok(file)
}
fn name(value: &str) -> Result<()> {
    if value.is_empty()
        || value == "."
        || value == ".."
        || value.contains(['/', '\\', ':'])
        || value.chars().any(char::is_control)
    {
        Err(failure())
    } else {
        Ok(())
    }
}
#[derive(PartialEq, Eq)]
pub struct Identity(u32, u32, u32);
pub struct Dir {
    path: PathBuf,
    held: Vec<Rc<Handle>>,
}
impl Dir {
    pub fn absolute(path: &Path) -> Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(failure());
        }
        let mut ancestors: Vec<_> = path.ancestors().collect();
        ancestors.reverse();
        let mut held = vec![];
        for part in ancestors {
            held.push(Rc::new(open(part, 0x80, 3, 3, true)?));
        }
        Ok(Self {
            path: path.into(),
            held,
        })
    }
    fn handle(&self) -> &Handle {
        self.held.last().unwrap()
    }
    pub fn identity(&self) -> Result<Identity> {
        let i = info(self.handle(), true)?;
        Ok(Identity(i.volume, i.index_high, i.index_low))
    }
    pub fn assert_path(&self, path: &Path) -> Result<()> {
        if self.identity()? != Self::absolute(path)?.identity()? {
            Err(failure())
        } else {
            Ok(())
        }
    }
    pub fn existing_or_make(&self, value: &str) -> Result<Self> {
        name(value)?;
        let path = self.path.join(value);
        if !self.exists(value)? {
            let sd = descriptor()?;
            let sa = Security {
                size: std::mem::size_of::<Security>() as u32,
                descriptor: sd.0,
                inherit: 0,
            };
            if unsafe { CreateDirectoryW(wide(&path)?.as_ptr(), &sa) } == 0 {
                return Err(failure());
            }
        }
        let file = Rc::new(open(&path, 0x80, 3, 3, true)?);
        private(&file)?;
        let mut held = self.held.clone();
        held.push(file);
        Ok(Self { path, held })
    }
    pub fn exists(&self, value: &str) -> Result<bool> {
        name(value)?;
        let attrs = unsafe { GetFileAttributesW(wide(&self.path.join(value))?.as_ptr()) };
        if attrs != u32::MAX {
            Ok(true)
        } else if unsafe { GetLastError() } == 2 {
            Ok(false)
        } else {
            Err(failure())
        }
    }
    pub fn lock(&self, value: &str) -> Result<Handle> {
        name(value)?;
        private(self.handle())?;
        let file = open(&self.path.join(value), 0xc0000000, 0, 4, false)?;
        private(&file)?;
        Ok(file)
    }
    pub fn read(&self, value: &str, limit: u64) -> Result<Vec<u8>> {
        name(value)?;
        private(self.handle())?;
        let file = open(&self.path.join(value), 0x80000000, 1, 3, false)?;
        private(&file)?;
        let before = info(&file, false)?;
        if ((before.high as u64) << 32 | before.low as u64) > limit {
            return Err(failure());
        }
        let mut bytes = vec![0u8; (limit + 1) as usize];
        let mut read = 0;
        if unsafe {
            ReadFile(
                file.0,
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                &mut read,
                ptr::null_mut(),
            )
        } == 0
            || read as u64 > limit
        {
            return Err(failure());
        }
        bytes.truncate(read as usize);
        let after = info(&file, false)?;
        if before.high != after.high || before.low != after.low || before.written != after.written {
            return Err(failure());
        }
        Ok(bytes)
    }
    pub fn atomic(&self, value: &str, bytes: &[u8]) -> Result<()> {
        name(value)?;
        private(self.handle())?;
        if self.exists(value)? {
            self.read(value, 1024 * 1024)?;
        }
        let temp = self
            .path
            .join(format!(".profiles-{:032x}", rand::random::<u128>()));
        {
            let file = open(&temp, 0x40000000, 0, 1, false)?;
            private(&file)?;
            let mut written = 0;
            if bytes.len() > u32::MAX as usize
                || unsafe {
                    WriteFile(
                        file.0,
                        bytes.as_ptr(),
                        bytes.len() as u32,
                        &mut written,
                        ptr::null_mut(),
                    )
                } == 0
                || written as usize != bytes.len()
                || unsafe { FlushFileBuffers(file.0) } == 0
            {
                return Err(failure());
            }
        }
        if unsafe {
            MoveFileExW(
                wide(&temp)?.as_ptr(),
                wide(&self.path.join(value))?.as_ptr(),
                9,
            )
        } == 0
        {
            return Err(failure());
        }
        Ok(())
    }
}
