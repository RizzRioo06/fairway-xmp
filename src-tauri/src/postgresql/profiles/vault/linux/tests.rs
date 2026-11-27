use super::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::{Mutex, OnceLock},
};
#[derive(Default)]
struct Object {
    attributes: HashMap<String, String>,
    bytes: Vec<u8>,
    id: String,
}
static VALUES: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
static FAIL: AtomicBool = AtomicBool::new(false);
static FAIL_DELETE: AtomicBool = AtomicBool::new(false);
static FAIL_STORE: AtomicBool = AtomicBool::new(false);
static FREED: AtomicUsize = AtomicUsize::new(0);
fn object(value: Object) -> P {
    Box::into_raw(Box::new(value)).cast()
}
unsafe fn string(value: P) -> String {
    unsafe { std::ffi::CStr::from_ptr(value.cast()) }
        .to_str()
        .unwrap()
        .into()
}
unsafe fn id(attributes: P) -> String {
    let attributes = unsafe { &*attributes.cast::<Object>() };
    assert_eq!(
        attributes.attributes.get("application").unwrap(),
        "dev.fairway.external-postgresql.v1"
    );
    attributes.attributes.get("account").unwrap().clone()
}
unsafe extern "C" fn service(flags: i32, _: P, _: E) -> P {
    assert_eq!(flags, 1);
    if FAIL.load(Ordering::SeqCst) {
        ptr::null_mut()
    } else {
        object(Object::default())
    }
}
unsafe extern "C" fn hash_new(
    _: unsafe extern "C" fn(P) -> u32,
    _: unsafe extern "C" fn(P, P) -> i32,
) -> P {
    object(Object::default())
}
unsafe extern "C" fn hash(_: P) -> u32 {
    0
}
unsafe extern "C" fn equal(_: P, _: P) -> i32 {
    1
}
unsafe extern "C" fn insert(p: P, key: P, value: P) -> i32 {
    unsafe { &mut *p.cast::<Object>() }
        .attributes
        .insert(unsafe { string(key) }, unsafe { string(value) });
    1
}
unsafe extern "C" fn free(p: P) {
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p.cast::<Object>()) });
    }
}
unsafe extern "C" fn value_free(p: P) {
    FREED.fetch_add(1, Ordering::SeqCst);
    unsafe { free(p) };
}
unsafe extern "C" fn value_new(bytes: *const c_char, size: isize, _: *const c_char) -> P {
    object(Object {
        bytes: unsafe { std::slice::from_raw_parts(bytes.cast(), size as usize) }.to_vec(),
        ..Default::default()
    })
}
unsafe extern "C" fn value_get(value: P, size: *mut usize) -> *const c_char {
    let value = unsafe { &*value.cast::<Object>() };
    unsafe { *size = value.bytes.len() };
    value.bytes.as_ptr().cast()
}
unsafe extern "C" fn store(
    _: P,
    schema: P,
    attributes: P,
    collection: *const c_char,
    _: *const c_char,
    value: P,
    _: P,
    _: E,
) -> i32 {
    if FAIL_STORE.load(Ordering::SeqCst) {
        return 0;
    }
    assert!(schema.is_null());
    assert_eq!(unsafe { string(collection.cast_mut().cast()) }, "default");
    VALUES.get_or_init(Default::default).lock().unwrap().insert(
        unsafe { id(attributes) },
        unsafe { &*value.cast::<Object>() }.bytes.clone(),
    );
    1
}
unsafe extern "C" fn lookup(_: P, schema: P, attributes: P, _: P, _: E) -> P {
    assert!(schema.is_null());
    let values = VALUES.get_or_init(Default::default).lock().unwrap();
    values
        .get(&unsafe { id(attributes) })
        .map(|v| {
            object(Object {
                bytes: v.clone(),
                ..Default::default()
            })
        })
        .unwrap_or(ptr::null_mut())
}
unsafe extern "C" fn search(_: P, schema: P, attributes: P, flags: i32, _: P, _: E) -> *mut List {
    assert!(schema.is_null());
    assert_eq!(flags, 2);
    let id = unsafe { id(attributes) };
    if !VALUES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .contains_key(&id)
    {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(List {
        data: object(Object {
            id,
            ..Default::default()
        }),
        next: ptr::null_mut(),
        previous: ptr::null_mut(),
    }))
}
unsafe extern "C" fn delete(item: P, _: P, _: E) -> i32 {
    if FAIL_DELETE.load(Ordering::SeqCst) {
        return 0;
    }
    VALUES
        .get_or_init(Default::default)
        .lock()
        .unwrap()
        .remove(&unsafe { &*item.cast::<Object>() }.id);
    1
}
unsafe extern "C" fn list_free(p: *mut List) {
    if !p.is_null() {
        drop(unsafe { Box::from_raw(p) });
    }
}
unsafe extern "C" fn cancel_new() -> P {
    object(Object::default())
}
unsafe extern "C" fn cancel(_: P) {}
#[test]
fn secret_service_api_roundtrip_namespacing_cleanup_and_unavailable_service() {
    let _: fn() -> Result<Api> = Api::load; // Type-check the loader without accessing a real service.
    let api = Api {
        service,
        store,
        search,
        lookup,
        delete,
        value_new,
        value_get,
        value_free,
        hash_new,
        hash,
        equal,
        insert,
        hash_free: free,
        object_free: free,
        error_free: free,
        list_free,
        cancel_new,
        cancel,
    };
    let id = "e".repeat(32);
    assert!(create(api, "invalid", &Password::from_bytes(b"fixture").unwrap()).is_err());
    create(
        api,
        &id,
        &Password::from_bytes("fixture 日本".as_bytes()).unwrap(),
    )
    .unwrap();
    assert!(read(api, &id).unwrap().bytes() == "fixture 日本".as_bytes());
    assert!(create(api, &id, &Password::from_bytes(b"overwrite").unwrap()).is_err());
    FAIL_DELETE.store(true, Ordering::SeqCst);
    assert!(remove(api, &id).is_err());
    assert!(read(api, &id).is_ok());
    FAIL_DELETE.store(false, Ordering::SeqCst);
    remove(api, &id).unwrap();
    remove(api, &id).unwrap();
    FAIL_STORE.store(true, Ordering::SeqCst);
    assert!(create(
        api,
        &"f".repeat(32),
        &Password::from_bytes(b"fixture").unwrap()
    )
    .is_err());
    FAIL_STORE.store(false, Ordering::SeqCst);
    assert!(read(api, &id).is_err());
    create(api, &id, &Password::from_bytes(b"").unwrap()).unwrap();
    assert!(read(api, &id).unwrap().bytes().is_empty());
    FAIL.store(true, Ordering::SeqCst);
    assert!(read(api, &id).is_err());
    assert!(remove(api, &id).is_err());
    assert!(create(
        api,
        &"f".repeat(32),
        &Password::from_bytes(b"fixture").unwrap()
    )
    .is_err());
    FAIL.store(false, Ordering::SeqCst);
    remove(api, &id).unwrap();
    assert!(FREED.load(Ordering::SeqCst) >= 4);
}
