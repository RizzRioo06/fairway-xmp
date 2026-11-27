//! Load only an independently verified client path; never search a project or PATH.
use super::super::explorer_contract::*;
use std::{
    ffi::{c_void, CStr},
    path::{Path, PathBuf},
};
type Ptr = *mut c_void;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub fn path(runtime: &Path) -> PathBuf {
    runtime.join("lib/libpq.5.dylib")
}
#[cfg(windows)]
pub fn path(runtime: &Path) -> PathBuf {
    runtime.join("libpq.dll")
}
pub fn open(path: &Path) -> Result<Ptr> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let p = {
        let s = std::ffi::CString::new(
            path.to_str()
                .ok_or_else(|| fail(Code::Unavailable, "Pinned client unavailable"))?,
        )
        .map_err(|_| fail(Code::Unavailable, "Pinned client unavailable"))?;
        unsafe { libc::dlopen(s.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) }
    };
    #[cfg(windows)]
    let p = {
        use std::os::windows::ffi::OsStrExt;
        let s: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            windows_sys::Win32::System::LibraryLoader::LoadLibraryExW(
                s.as_ptr(),
                std::ptr::null_mut(),
                0x100 | 0x800,
            )
        }
    };
    if p.is_null() {
        Err(fail(
            Code::Unavailable,
            "Pinned PostgreSQL client unavailable",
        ))
    } else {
        Ok(p)
    }
}
pub unsafe fn get<T: Copy>(library: Ptr, name: &CStr) -> Result<T> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let p = unsafe { libc::dlsym(library, name.as_ptr()) };
    #[cfg(windows)]
    let p = unsafe {
        windows_sys::Win32::System::LibraryLoader::GetProcAddress(library, name.as_ptr().cast())
    }
    .map(|f| f as *const () as Ptr)
    .unwrap_or(std::ptr::null_mut());
    if p.is_null() {
        Err(fail(
            Code::Unavailable,
            "Pinned PostgreSQL client API unavailable",
        ))
    } else {
        Ok(unsafe { std::mem::transmute_copy(&p) })
    }
}
pub unsafe fn close(library: Ptr) {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    unsafe {
        libc::dlclose(library);
    }
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::Foundation::FreeLibrary(library);
    }
}
