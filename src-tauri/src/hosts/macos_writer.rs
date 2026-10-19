//! Fixed-capability C ABI for the signed launch daemon; no caller-supplied paths.
use std::ffi::{c_char, CString};

fn decode(bytes: &[u8]) -> crate::config::Result<Vec<String>> {
    if bytes.len() > 65536 {
        return Err("Address request is too large.".into());
    }
    let names: Vec<String> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if names.len() > 1024
        || names
            .iter()
            .any(|name| !crate::config::valid_project_name(name))
    {
        return Err("Invalid site domains or too many sites (maximum 1024).".into());
    }
    Ok(names)
}

/// # Safety
/// `bytes` must point to `length` readable bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn fairway_hosts_apply(bytes: *const u8, length: usize) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        if bytes.is_null() || length > 65536 {
            return Err("Invalid address request.".to_string());
        }
        let names = decode(unsafe { std::slice::from_raw_parts(bytes, length) })?;
        if unsafe { libc::geteuid() } != 0 {
            return Err("The address helper must run through launchd.".into());
        }
        super::apply(&names)
    })
    .unwrap_or_else(|_| Err("Address helper failed.".into()));
    match result {
        Ok(()) => std::ptr::null_mut(),
        Err(message) => CString::new(message.replace('\0', " ")).unwrap().into_raw(),
    }
}

/// # Safety
/// `error` must be a non-null pointer returned by `fairway_hosts_apply`, freed once.
#[no_mangle]
pub unsafe extern "C" fn fairway_hosts_free_error(error: *mut c_char) {
    if !error.is_null() {
        drop(unsafe { CString::from_raw(error) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_bounded_local_labels() {
        assert_eq!(decode(br#"["shop", "my-site"]"#).unwrap().len(), 2);
        for invalid in [
            br#"["shop.test"]"#.as_slice(),
            br#"["../../hosts"]"#,
            br#"{"path":"/etc/passwd"}"#,
            br#"["a\n127.0.0.1 other"]"#,
        ] {
            assert!(decode(invalid).is_err());
        }
        assert!(decode(&vec![b' '; 65537]).is_err());
        assert!(decode(&serde_json::to_vec(&vec!["a"; 1025]).unwrap()).is_err());
        // Exercise the ABI without touching the real hosts file, even under root.
        let error = unsafe { fairway_hosts_apply(std::ptr::null(), 0) };
        assert!(!error.is_null());
        unsafe { fairway_hosts_free_error(error) };
    }
}
