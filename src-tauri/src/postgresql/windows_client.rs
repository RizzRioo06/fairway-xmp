//! External Windows clients reuse only the build-pinned PHP libpq/DLL closure.
use super::explorer_contract::*;
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    platform: String,
    source_sha256: String,
    files: BTreeMap<String, Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    sha256: String,
    bytes: u64,
}
fn unavailable() -> Error {
    fail(Code::Unavailable,"Verified Windows PostgreSQL connection tools are unavailable. Use a matching Windows app build with its bundled runtime")
}
fn hash(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn parse(bytes: &[u8]) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(|_| unavailable())?;
    let lock: serde_json::Value =
        serde_json::from_str(include_str!("../../../runtime-downloads.lock.json"))
            .map_err(|_| unavailable())?;
    if manifest.schema_version != 1
        || manifest.platform != "windows-x64"
        || Some(manifest.source_sha256.as_str()) != lock["php-windows"]["sha256"].as_str()
        || manifest.files.is_empty()
        || manifest.files.len() > 128
        || !manifest.files.contains_key("libpq.dll")
    {
        return Err(unavailable());
    }
    let mut names = std::collections::HashSet::new();
    let mut total = 0u64;
    for (name, e) in &manifest.files {
        if name.is_empty()
            || name.contains(['/', '\\', ':'])
            || name.chars().any(char::is_control)
            || !name.to_ascii_lowercase().ends_with(".dll")
            || !names.insert(name.to_ascii_lowercase())
            || e.bytes == 0
            || e.bytes > 128 * 1024 * 1024
            || e.sha256.len() != 64
            || !e.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(unavailable());
        }
        total += e.bytes;
    }
    if total > 512 * 1024 * 1024 {
        return Err(unavailable());
    }
    Ok(manifest)
}
#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        fs::{File, OpenOptions},
        io::Read,
        os::windows::{
            fs::{MetadataExt, OpenOptionsExt},
            io::AsRawHandle,
        },
        path::{Component, Path},
    };
    /// Shared-read handles deny modification/deletion until this operation ends.
    pub struct Lease {
        _files: Vec<File>,
    }
    fn open(path: &Path, directory: bool) -> Result<File> {
        let mut o = OpenOptions::new();
        o.read(true)
            .share_mode(1 | if directory { 2 } else { 0 })
            .custom_flags(0x00200000 | if directory { 0x02000000 } else { 0 });
        let f = o.open(path).map_err(|_| unavailable())?;
        let mut info = std::mem::MaybeUninit::zeroed();
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandle(
                f.as_raw_handle() as _,
                info.as_mut_ptr(),
            )
        } == 0
        {
            return Err(unavailable());
        }
        let info = unsafe { info.assume_init() };
        if !directory && info.nNumberOfLinks != 1 {
            return Err(unavailable());
        }
        let m = f.metadata().map_err(|_| unavailable())?;
        if m.file_attributes() & 0x400 != 0 || m.is_dir() != directory || !directory && !m.is_file()
        {
            return Err(unavailable());
        }
        Ok(f)
    }
    pub fn verify(path: &Path) -> Result<Lease> {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(unavailable());
        }
        let m = parse(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/windows-postgresql-client.json"
        )))?;
        let mut files = vec![];
        let mut ancestors: Vec<_> = path.ancestors().collect();
        ancestors.reverse();
        for dir in ancestors {
            files.push(open(dir, true)?);
        }
        for entry in std::fs::read_dir(path).map_err(|_| unavailable())? {
            let entry = entry.map_err(|_| unavailable())?;
            let name = entry.file_name().into_string().map_err(|_| unavailable())?;
            if name.to_ascii_lowercase().ends_with(".dll") && !m.files.contains_key(&name) {
                return Err(unavailable());
            }
        }
        for (name, expected) in m.files {
            let mut f = open(&path.join(name), false)?;
            let meta = f.metadata().map_err(|_| unavailable())?;
            if meta.len() != expected.bytes {
                return Err(unavailable());
            }
            let mut bytes = Vec::new();
            (&mut f)
                .take(expected.bytes + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| unavailable())?;
            if bytes.len() as u64 != expected.bytes || hash(&bytes) != expected.sha256 {
                return Err(unavailable());
            }
            files.push(f);
        }
        Ok(Lease { _files: files })
    }
}
#[cfg(windows)]
pub(crate) use platform::verify;
#[cfg(windows)]
pub(crate) fn connection(
    layout: Option<&crate::config::Layout>,
    cluster: Option<&str>,
    database: String,
) -> Result<super::explorer::wire::Envelope> {
    if cluster.is_some() {
        return Err(fail(Code::Unavailable,"Managed PostgreSQL servers are not available on Windows; connect to an existing server"));
    }
    let runtime = layout.ok_or_else(unavailable)?.runtime.join("php/bin");
    let _lease = verify(&runtime)?;
    Ok(super::explorer::wire::Envelope {
        runtime,
        host: "127.0.0.1".into(),
        port: 1,
        user: "postgres".into(),
        database,
        password: Password::from_bytes(b"")?,
        managed: None,
        operation: Operation::Schemas,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> serde_json::Value {
        let lock: serde_json::Value =
            serde_json::from_str(include_str!("../../../runtime-downloads.lock.json")).unwrap();
        serde_json::json!({"schemaVersion":1,"platform":"windows-x64","sourceSha256":lock["php-windows"]["sha256"],"files":{"libpq.dll":{"sha256":hash(b"fixture"),"bytes":7}}})
    }
    #[test]
    fn windows_pin_requires_reviewed_archive_safe_names_and_bounded_dlls() {
        assert!(parse(&serde_json::to_vec(&manifest()).unwrap()).is_ok());
        for key in ["../libpq.dll", "X:libpq.dll", "sub\\libpq.dll", "LIBPQ.DLL"] {
            let mut m = manifest();
            m["files"][key] = m["files"]["libpq.dll"].clone();
            assert!(parse(&serde_json::to_vec(&m).unwrap()).is_err());
        }
        let mut m = manifest();
        m["sourceSha256"] = serde_json::json!("0".repeat(64));
        assert!(parse(&serde_json::to_vec(&m).unwrap()).is_err());
        let mut m = manifest();
        m["files"]["libpq.dll"]["bytes"] = serde_json::json!(0);
        assert!(parse(&serde_json::to_vec(&m).unwrap()).is_err());
        assert!(parse(b"null").is_err());
    }
}
