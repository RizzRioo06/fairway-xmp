//! libsecret's direct Secret Service API. No CLI or alternative file backend.
use super::*;
use std::{
    ffi::{c_char, c_void, CString},
    ptr,
};
type P = *mut c_void;
type E = *mut P;
#[repr(C)]
struct List {
    data: P,
    next: *mut List,
    previous: *mut List,
}
#[derive(Clone, Copy)]
struct Api {
    service: unsafe extern "C" fn(i32, P, E) -> P,
    store: unsafe extern "C" fn(P, P, P, *const c_char, *const c_char, P, P, E) -> i32,
    search: unsafe extern "C" fn(P, P, P, i32, P, E) -> *mut List,
    lookup: unsafe extern "C" fn(P, P, P, P, E) -> P,
    delete: unsafe extern "C" fn(P, P, E) -> i32,
    value_new: unsafe extern "C" fn(*const c_char, isize, *const c_char) -> P,
    value_get: unsafe extern "C" fn(P, *mut usize) -> *const c_char,
    value_free: unsafe extern "C" fn(P),
    hash_new: unsafe extern "C" fn(
        unsafe extern "C" fn(P) -> u32,
        unsafe extern "C" fn(P, P) -> i32,
    ) -> P,
    hash: unsafe extern "C" fn(P) -> u32,
    equal: unsafe extern "C" fn(P, P) -> i32,
    insert: unsafe extern "C" fn(P, P, P) -> i32,
    hash_free: unsafe extern "C" fn(P),
    object_free: unsafe extern "C" fn(P),
    error_free: unsafe extern "C" fn(P),
    list_free: unsafe extern "C" fn(*mut List),
    cancel_new: unsafe extern "C" fn() -> P,
    cancel: unsafe extern "C" fn(P),
}
fn unavailable() -> Error {
    fail(Code::Unavailable, "Linux Secret Service is unavailable, locked or the request was cancelled. Unlock a Secret Service keyring or use a session-only password; no plaintext fallback was used")
}
#[cfg(any(target_os = "linux", test))]
impl Api {
    fn load() -> Result<Self> {
        use std::{ffi::CStr, os::unix::fs::MetadataExt};
        static API: std::sync::OnceLock<Option<Api>> = std::sync::OnceLock::new();
        API.get_or_init(|| {
            // Only trusted system paths; don't load from a project, HOME or LD_LIBRARY_PATH.
            let candidates = [
                "/usr/lib/x86_64-linux-gnu/libsecret-1.so.0",
                "/usr/lib/aarch64-linux-gnu/libsecret-1.so.0",
                "/usr/lib64/libsecret-1.so.0",
                "/usr/lib/libsecret-1.so.0",
            ];
            for path in candidates {
                let Ok(path) = std::fs::canonicalize(path) else {
                    continue;
                };
                let mut safe = true;
                for ancestor in path.ancestors() {
                    let Ok(meta) = std::fs::symlink_metadata(ancestor) else {
                        safe = false;
                        break;
                    };
                    if meta.uid() != 0 || meta.mode() & 0o022 != 0 {
                        safe = false;
                        break;
                    }
                }
                if !safe {
                    continue;
                }
                let Ok(path) = CString::new(path.as_os_str().as_encoded_bytes()) else {
                    continue;
                };
                let library =
                    unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
                if library.is_null() {
                    continue;
                }
                unsafe fn symbol<T: Copy>(lib: P, name: &CStr) -> Option<T> {
                    let p = unsafe { libc::dlsym(lib, name.as_ptr()) };
                    if p.is_null() {
                        None
                    } else {
                        Some(unsafe { std::mem::transmute_copy(&p) })
                    }
                }
                // Keep libsecret resident: GLib types/proxies can outlive a single call.
                let result = (|| {
                    Some(Api {
                        service: unsafe { symbol(library, c"secret_service_get_sync")? },
                        store: unsafe { symbol(library, c"secret_service_store_sync")? },
                        search: unsafe { symbol(library, c"secret_service_search_sync")? },
                        lookup: unsafe { symbol(library, c"secret_service_lookup_sync")? },
                        delete: unsafe { symbol(library, c"secret_item_delete_sync")? },
                        value_new: unsafe { symbol(library, c"secret_value_new")? },
                        value_get: unsafe { symbol(library, c"secret_value_get")? },
                        value_free: unsafe { symbol(library, c"secret_value_unref")? },
                        hash_new: unsafe { symbol(library, c"g_hash_table_new")? },
                        hash: unsafe { symbol(library, c"g_str_hash")? },
                        equal: unsafe { symbol(library, c"g_str_equal")? },
                        insert: unsafe { symbol(library, c"g_hash_table_insert")? },
                        hash_free: unsafe { symbol(library, c"g_hash_table_unref")? },
                        object_free: unsafe { symbol(library, c"g_object_unref")? },
                        error_free: unsafe { symbol(library, c"g_error_free")? },
                        list_free: unsafe { symbol(library, c"g_list_free")? },
                        cancel_new: unsafe { symbol(library, c"g_cancellable_new")? },
                        cancel: unsafe { symbol(library, c"g_cancellable_cancel")? },
                    })
                })();
                if result.is_some() {
                    return result;
                }
                unsafe { libc::dlclose(library) };
            }
            None
        })
        .as_ref()
        .copied()
        .ok_or_else(unavailable)
    }
}
struct Owned {
    p: P,
    free: unsafe extern "C" fn(P),
}
impl Owned {
    fn new(p: P, free: unsafe extern "C" fn(P)) -> Result<Self> {
        if p.is_null() {
            Err(unavailable())
        } else {
            Ok(Self { p, free })
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { (self.free)(self.p) };
    }
}
struct Items {
    p: *mut List,
    api: Api,
}
impl Drop for Items {
    fn drop(&mut self) {
        let mut item = self.p;
        while !item.is_null() {
            unsafe {
                (self.api.object_free)((*item).data);
                item = (*item).next;
            }
        }
        unsafe { (self.api.list_free)(self.p) };
    }
}
struct Cancel {
    object: Owned,
    done: std::sync::mpsc::Sender<()>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Cancel {
    fn new(api: Api) -> Result<Self> {
        let object = Owned::new(unsafe { (api.cancel_new)() }, api.object_free)?;
        let pointer = object.p as usize;
        let (done, receiver) = std::sync::mpsc::channel();
        let cancel = api.cancel;
        let thread = std::thread::spawn(move || {
            if receiver
                .recv_timeout(std::time::Duration::from_secs(30))
                .is_err()
            {
                unsafe { cancel(pointer as P) }
            }
        });
        Ok(Self {
            object,
            done,
            thread: Some(thread),
        })
    }
}
impl Drop for Cancel {
    fn drop(&mut self) {
        let _ = self.done.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
fn check(api: Api, error: P) -> Result<()> {
    if error.is_null() {
        Ok(())
    } else {
        unsafe { (api.error_free)(error) };
        Err(unavailable())
    }
}
fn with<T>(api: Api, id: &str, action: impl FnOnce(P, P, P) -> Result<T>) -> Result<T> {
    validate_id(id)?;
    let id = CString::new(id).map_err(|_| unavailable())?;
    let cancel = Cancel::new(api)?;
    let mut error = ptr::null_mut();
    let raw = unsafe { (api.service)(1, cancel.object.p, &mut error) };
    // Own returned objects even if an implementation reports an error as well.
    let service = (!raw.is_null()).then(|| Owned {
        p: raw,
        free: api.object_free,
    });
    check(api, error)?;
    let service = service.ok_or_else(unavailable)?;
    let attributes = Owned::new(
        unsafe { (api.hash_new)(api.hash, api.equal) },
        api.hash_free,
    )?;
    unsafe {
        (api.insert)(
            attributes.p,
            c"application".as_ptr().cast_mut().cast(),
            c"dev.fairway.external-postgresql.v1"
                .as_ptr()
                .cast_mut()
                .cast(),
        );
        (api.insert)(
            attributes.p,
            c"account".as_ptr().cast_mut().cast(),
            id.as_ptr().cast_mut().cast(),
        );
    }
    action(service.p, attributes.p, cancel.object.p)
}
fn search(api: Api, service: P, attributes: P, cancel: P) -> Result<Items> {
    let mut error = ptr::null_mut();
    let p = unsafe { (api.search)(service, ptr::null_mut(), attributes, 2, cancel, &mut error) };
    let list = Items { p, api };
    check(api, error)?;
    Ok(list)
}
fn create(api: Api, id: &str, password: &Password) -> Result<()> {
    validate_password(password)?;
    with(api, id, |service, attributes, cancel| {
        if !search(api, service, attributes, cancel)?.p.is_null() {
            return Err(fail(
                Code::Busy,
                "Saved credential identity is already occupied",
            ));
        }
        let value = Owned::new(
            unsafe {
                (api.value_new)(
                    password.bytes().as_ptr().cast(),
                    password.bytes().len() as isize,
                    c"text/plain; charset=utf-8".as_ptr(),
                )
            },
            api.value_free,
        )?;
        let mut error = ptr::null_mut();
        let result = unsafe {
            (api.store)(
                service,
                ptr::null_mut(),
                attributes,
                c"default".as_ptr(),
                c"Fairway PostgreSQL saved connection".as_ptr(),
                value.p,
                cancel,
                &mut error,
            )
        };
        check(api, error)?;
        if result == 0 {
            Err(unavailable())
        } else {
            Ok(())
        }
    })
}
fn read(api: Api, id: &str) -> Result<Password> {
    with(api, id, |service, attributes, cancel| {
        let mut error = ptr::null_mut();
        let raw = unsafe { (api.lookup)(service, ptr::null_mut(), attributes, cancel, &mut error) };
        let value = (!raw.is_null()).then(|| Owned {
            p: raw,
            free: api.value_free,
        });
        check(api, error)?;
        let value = value.ok_or_else(unavailable)?;
        let mut size = 0;
        let bytes = unsafe { (api.value_get)(value.p, &mut size) };
        if size > 4096 || bytes.is_null() {
            return Err(unavailable());
        }
        let p = Password::from_bytes(unsafe { std::slice::from_raw_parts(bytes.cast(), size) })?;
        validate_password(&p)?;
        Ok(p)
    })
}
fn remove(api: Api, id: &str) -> Result<()> {
    with(api, id, |service, attributes, cancel| {
        // Explicit item deletion includes locked items and reports cancelled prompts.
        // clear_sync alone can report no error while leaving locked items behind.
        let list = search(api, service, attributes, cancel)?;
        let mut item = list.p;
        while !item.is_null() {
            let mut error = ptr::null_mut();
            let result = unsafe { (api.delete)((*item).data, cancel, &mut error) };
            check(api, error)?;
            if result == 0 {
                return Err(unavailable());
            }
            item = unsafe { (*item).next };
        }
        Ok(())
    })
}
#[cfg(target_os = "linux")]
pub(super) struct SecretService;
#[cfg(target_os = "linux")]
impl Vault for SecretService {
    fn create(&self, id: &str, password: &Password) -> Result<()> {
        create(Api::load()?, id, password)
    }
    fn read(&self, id: &str) -> Result<Password> {
        read(Api::load()?, id)
    }
    fn remove(&self, id: &str) -> Result<()> {
        remove(Api::load()?, id)
    }
}
#[cfg(test)]
mod tests;
