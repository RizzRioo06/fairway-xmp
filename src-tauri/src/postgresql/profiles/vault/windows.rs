//! Windows generic credentials, scoped to this user's local machine logon store.
use super::*;
use std::{ffi::c_void, ptr};
#[repr(C)]
#[derive(Default)]
struct Credential {
    flags: u32,
    kind: u32,
    target: *mut u16,
    comment: *mut u16,
    written: [u32; 2],
    size: u32,
    blob: *mut u8,
    persist: u32,
    attribute_count: u32,
    attributes: *mut c_void,
    alias: *mut u16,
    user: *mut u16,
}
#[derive(Clone, Copy)]
struct Api {
    read: unsafe extern "system" fn(*const u16, u32, u32, *mut *mut Credential) -> i32,
    write: unsafe extern "system" fn(*const Credential, u32) -> i32,
    delete: unsafe extern "system" fn(*const u16, u32, u32) -> i32,
    free: unsafe extern "system" fn(*mut c_void),
    error: unsafe extern "system" fn() -> u32,
}
#[cfg(windows)]
impl Api {
    fn native() -> Self {
        #[link(name = "Advapi32")]
        unsafe extern "system" {
            fn CredReadW(
                target: *const u16,
                kind: u32,
                flags: u32,
                out: *mut *mut Credential,
            ) -> i32;
            fn CredWriteW(value: *const Credential, flags: u32) -> i32;
            fn CredDeleteW(target: *const u16, kind: u32, flags: u32) -> i32;
            fn CredFree(value: *mut c_void);
        }
        #[link(name = "Kernel32")]
        unsafe extern "system" {
            fn GetLastError() -> u32;
        }
        Self {
            read: CredReadW,
            write: CredWriteW,
            delete: CredDeleteW,
            free: CredFree,
            error: GetLastError,
        }
    }
}
fn unavailable() -> Error {
    fail(Code::Unavailable, "Windows Credential Manager is unavailable. Unlock your Windows account or use a session-only password; no plaintext fallback was used")
}
fn target(id: &str) -> Result<Vec<u16>> {
    validate_id(id)?;
    Ok(format!("dev.fairway.external-postgresql.v1/{id}")
        .encode_utf16()
        .chain(Some(0))
        .collect())
}
struct OwnedCredential {
    pointer: *mut Credential,
    api: Api,
}
impl Drop for OwnedCredential {
    fn drop(&mut self) {
        unsafe {
            let value = &*self.pointer;
            if !value.blob.is_null() && value.size <= 5120 {
                for byte in std::slice::from_raw_parts_mut(value.blob, value.size as usize) {
                    ptr::write_volatile(byte, 0);
                }
            }
            (self.api.free)(self.pointer.cast());
        }
    }
}
fn lookup(api: Api, target: &[u16]) -> Result<Option<OwnedCredential>> {
    let mut pointer = ptr::null_mut();
    if unsafe { (api.read)(target.as_ptr(), 1, 0, &mut pointer) } == 0 {
        return if unsafe { (api.error)() } == 1168 {
            Ok(None)
        } else {
            Err(unavailable())
        };
    }
    if pointer.is_null() {
        return Err(unavailable());
    }
    Ok(Some(OwnedCredential { pointer, api }))
}
fn create(api: Api, id: &str, password: &Password) -> Result<()> {
    validate_password(password)?;
    let mut name = target(id)?;
    // Random revision IDs are immutable; don't silently replace a pre-existing item.
    if lookup(api, &name)?.is_some() {
        return Err(fail(
            Code::Busy,
            "Saved credential identity is already occupied",
        ));
    }
    let mut user: Vec<u16> = "Fairway PostgreSQL".encode_utf16().chain(Some(0)).collect();
    let value = Credential {
        kind: 1,
        target: name.as_mut_ptr(),
        size: password.bytes().len() as u32,
        blob: password.bytes().as_ptr().cast_mut(),
        persist: 2,
        user: user.as_mut_ptr(),
        ..Default::default()
    };
    if unsafe { (api.write)(&value, 0) } == 0 {
        Err(unavailable())
    } else {
        Ok(())
    }
}
fn read(api: Api, id: &str) -> Result<Password> {
    let value = lookup(api, &target(id)?)?.ok_or_else(unavailable)?;
    let raw = unsafe { &*value.pointer };
    if raw.kind != 1 || raw.size > 4096 || raw.blob.is_null() && raw.size != 0 {
        return Err(unavailable());
    }
    let bytes = if raw.size == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(raw.blob, raw.size as usize) }
    };
    let password = Password::from_bytes(bytes)?;
    validate_password(&password)?;
    Ok(password)
}
fn remove(api: Api, id: &str) -> Result<()> {
    let target = target(id)?;
    if unsafe { (api.delete)(target.as_ptr(), 1, 0) } != 0 || unsafe { (api.error)() } == 1168 {
        Ok(())
    } else {
        Err(unavailable())
    }
}
#[cfg(windows)]
pub(super) struct CredentialManager;
#[cfg(windows)]
impl Vault for CredentialManager {
    fn create(&self, id: &str, password: &Password) -> Result<()> {
        create(Api::native(), id, password)
    }
    fn read(&self, id: &str) -> Result<Password> {
        read(Api::native(), id)
    }
    fn remove(&self, id: &str) -> Result<()> {
        remove(Api::native(), id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicU32, Ordering},
        sync::{Mutex, OnceLock},
    };
    static VALUES: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    static ERROR: AtomicU32 = AtomicU32::new(1168);
    static WIPED: AtomicU32 = AtomicU32::new(0);
    unsafe fn name(p: *const u16) -> String {
        let mut n = 0;
        while unsafe { *p.add(n) } != 0 {
            n += 1;
        }
        String::from_utf16(unsafe { std::slice::from_raw_parts(p, n) }).unwrap()
    }
    unsafe extern "system" fn get(
        p: *const u16,
        kind: u32,
        flags: u32,
        out: *mut *mut Credential,
    ) -> i32 {
        assert_eq!((kind, flags), (1, 0));
        let values = VALUES.get_or_init(Default::default).lock().unwrap();
        let Some(bytes) = values.get(&unsafe { name(p) }) else {
            ERROR.store(1168, Ordering::SeqCst);
            return 0;
        };
        let blob = bytes.clone().into_boxed_slice();
        let size = blob.len() as u32;
        let pointer = Box::into_raw(blob).cast::<u8>();
        unsafe {
            *out = Box::into_raw(Box::new(Credential {
                kind: 1,
                size,
                blob: pointer,
                ..Default::default()
            }));
        }
        1
    }
    unsafe extern "system" fn put(p: *const Credential, flags: u32) -> i32 {
        let p = unsafe { &*p };
        assert_eq!((p.kind, p.persist, flags), (1, 2, 0));
        VALUES.get_or_init(Default::default).lock().unwrap().insert(
            unsafe { name(p.target) },
            unsafe { std::slice::from_raw_parts(p.blob, p.size as usize) }.to_vec(),
        );
        1
    }
    unsafe extern "system" fn delete(p: *const u16, _: u32, _: u32) -> i32 {
        VALUES
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .remove(&unsafe { name(p) });
        1
    }
    unsafe extern "system" fn free(p: *mut c_void) {
        let p = unsafe { Box::from_raw(p.cast::<Credential>()) };
        let blob = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(p.blob, p.size as usize)) };
        assert!(blob.iter().all(|b| *b == 0));
        WIPED.fetch_add(1, Ordering::SeqCst);
    }
    unsafe extern "system" fn error() -> u32 {
        ERROR.load(Ordering::SeqCst)
    }
    unsafe extern "system" fn denied_read(
        _: *const u16,
        _: u32,
        _: u32,
        _: *mut *mut Credential,
    ) -> i32 {
        ERROR.store(5, Ordering::SeqCst);
        0
    }
    unsafe extern "system" fn denied_write(_: *const Credential, _: u32) -> i32 {
        ERROR.store(5, Ordering::SeqCst);
        0
    }
    unsafe extern "system" fn denied_delete(_: *const u16, _: u32, _: u32) -> i32 {
        ERROR.store(5, Ordering::SeqCst);
        0
    }
    #[test]
    fn windows_abi_roundtrip_is_namespaced_local_only_wiped_and_idempotent() {
        let api = Api {
            read: get,
            write: put,
            delete,
            free,
            error,
        };
        let id = "d".repeat(32);
        assert!(read(
            Api {
                read: denied_read,
                ..api
            },
            &id
        )
        .is_err());
        assert!(create(
            Api {
                write: denied_write,
                ..api
            },
            &id,
            &Password::from_bytes(b"fixture").unwrap()
        )
        .is_err());
        assert!(remove(
            Api {
                delete: denied_delete,
                ..api
            },
            &id
        )
        .is_err());
        assert!(target("../invalid").is_err());
        assert!(read(api, &id).is_err());
        create(
            api,
            &id,
            &Password::from_bytes("fixture 日本".as_bytes()).unwrap(),
        )
        .unwrap();
        assert!(create(api, &id, &Password::from_bytes(b"replacement").unwrap()).is_err());
        assert!(read(api, &id).unwrap().bytes() == "fixture 日本".as_bytes());
        assert!(VALUES
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .keys()
            .all(|key| key.starts_with("dev.fairway.external-postgresql.v1/")));
        assert!(WIPED.load(Ordering::SeqCst) > 0);
        remove(api, &id).unwrap();
        remove(api, &id).unwrap();
        create(api, &id, &Password::from_bytes(b"").unwrap()).unwrap();
        assert!(read(api, &id).unwrap().bytes().is_empty());
        remove(api, &id).unwrap();
    }
}
