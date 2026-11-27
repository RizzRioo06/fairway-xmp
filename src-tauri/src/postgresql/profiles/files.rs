#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(super) use crate::postgresql::managed::files::Dir;
#[cfg(any(
    all(unix, not(all(target_os = "macos", target_arch = "aarch64"))),
    all(test, unix)
))]
mod unix;
#[cfg(all(unix, not(all(target_os = "macos", target_arch = "aarch64"))))]
pub(super) use unix::Dir;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(super) use windows::Dir;

#[cfg(all(test, unix))]
mod tests {
    use super::unix::Dir;
    use std::{
        fs,
        os::unix::fs::{symlink, PermissionsExt},
    };
    #[test]
    fn unix_profile_storage_preserves_private_atomic_locking_and_refuses_aliases() {
        let path = std::env::temp_dir().join(format!(
            "fairway-profile-storage-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        let path = fs::canonicalize(path).unwrap();
        {
            let parent = Dir::absolute(&path).unwrap();
            let dir = parent.existing_or_make("profiles").unwrap();
            let _lock = dir.lock(".lock").unwrap();
            assert!(dir.lock(".lock").is_err());
            dir.assert_path(&path.join("profiles")).unwrap();
            dir.atomic("profiles.json", b"first").unwrap();
            dir.atomic("profiles.json", b"second").unwrap();
            assert_eq!(dir.read("profiles.json", 100).unwrap(), b"second");
            assert!(dir.read("profiles.json", 1).is_err());
            fs::rename(path.join("profiles/profiles.json"), path.join("retained")).unwrap();
            symlink(path.join("retained"), path.join("profiles/profiles.json")).unwrap();
            assert!(dir.atomic("profiles.json", b"forbidden").is_err());
            assert_eq!(fs::read(path.join("retained")).unwrap(), b"second");
        }
        fs::remove_dir_all(path).unwrap();
    }
}
