//! Preserve a stopped database before the supported 8.4 → 9.7 LTS upgrade.
use crate::config::{atomic_write, Layout, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Deserialize, Serialize)]
struct Upgrade {
    from: String,
    to: String,
    backup: std::path::PathBuf,
}

pub fn runtime_family(layout: &Layout) -> Result<String> {
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(layout.runtime.join("manifest.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let version = manifest["mysql"]
        .as_str()
        .ok_or("The bundled MySQL version is missing.")?;
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.parse::<u32>().is_err()) {
        return Err("The bundled MySQL version is invalid.".into());
    }
    Ok(format!("{}.{}", parts[0], parts[1]))
}

pub fn prepare_upgrade(layout: &Layout) -> Result<()> {
    let data = layout.data.join("mysql");
    if !data.join("mysql").exists() {
        return Ok(());
    }
    let target = runtime_family(layout)?;
    let current = fs::read_to_string(layout.data.join("database-version"))
        .or_else(|_| fs::read_to_string(layout.data.join("database-ready")))
        .map_err(|_| "The existing database version could not be identified. Your data has been kept. Use the previous Fairway XMP version to export your databases.".to_string())?;
    let current = current.trim();
    if current == target {
        return Ok(());
    }
    if current != "8.4" || target != "9.7" {
        return Err(format!("This database uses MySQL {current} and cannot be opened by MySQL {target}. Your data has been kept. Use the matching Fairway XMP version to export it before switching versions."));
    }
    if fs::read_to_string(layout.data.join("mysql.pid"))
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .is_some_and(crate::process::pid_alive)
    {
        return Err("Stop the existing MySQL server before upgrading your databases.".into());
    }
    let journal = layout.data.join("database-upgrade.json");
    if journal.exists() {
        let upgrade: Upgrade = serde_json::from_slice(
            &fs::read(&journal).map_err(|e| e.to_string())?,
        )
        .map_err(|_| {
            "The database upgrade record could not be read. Your data and backup have been kept."
                .to_string()
        })?;
        if upgrade.from != current
            || upgrade.to != target
            || !upgrade.backup.join("mysql/mysql").is_dir()
        {
            return Err("The previous database upgrade backup could not be verified. Your data has been kept; check the backups folder before retrying.".into());
        }
        // A failed startup may already have changed the data dictionary. Keep the
        // original backup and retry the new server; never overwrite the backup.
        return crate::durability::sync_directory(&layout.data);
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let backup = layout
        .data
        .join(format!("backups/mysql-{current}-before-{target}-{stamp}"));
    let mut staged = backup.as_os_str().to_os_string();
    staged.push(".partial");
    let staged = std::path::PathBuf::from(staged);
    fs::create_dir_all(&staged)
        .map_err(|e| format!("Could not create the database upgrade backup: {e}"))?;
    copy_directory(&data, &staged.join("mysql"))
        .map_err(|e| format!("Could not finish the database upgrade backup: {e}. The existing database has not been opened."))?;
    for file in [
        "credentials",
        "database-ready",
        "database-version",
        "settings.json",
    ] {
        let source = layout.data.join(file);
        if source.exists() {
            fs::copy(source, staged.join(file)).map_err(|e| e.to_string())?;
        }
    }
    // Alpha 1 recorded its family in database-ready only. Include the explicit
    // version file in its backup so restoring cannot retain the newer marker.
    fs::write(staged.join("database-version"), current).map_err(|e| e.to_string())?;
    fs::write(staged.join("README.txt"),
        "This is a copy of your stopped MySQL 8.4 database before its upgrade to MySQL 9.7. Keep it until you have checked your databases. To restore it, quit Fairway XMP, use the previous version, and restore the mysql folder and the saved credentials, database-ready, database-version, and settings.json files together. Never open this backup with the newer MySQL server.\n")
        .map_err(|e| e.to_string())?;
    crate::durability::sync_tree(&staged)?;
    crate::durability::publish(&staged, &backup)?;
    atomic_write(
        &journal,
        &serde_json::to_vec_pretty(&Upgrade {
            from: current.into(),
            to: target,
            backup,
        })
        .map_err(|e| e.to_string())?,
    )?;
    crate::durability::sync_directory(&layout.data)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir(destination).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() || (!kind.is_dir() && !kind.is_file()) {
            return Err(
                "An unexpected link or special file is present in the database folder.".into(),
            );
        }
        if kind.is_dir() {
            copy_directory(&entry.path(), &destination.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), destination.join(entry.file_name()))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn interrupted_or_failed_copy_is_never_recorded_as_complete() {
        use std::os::unix::fs::symlink;
        let root =
            std::env::temp_dir().join(format!("fairway-upgrade-partial-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        layout.initialize().unwrap();
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            br#"{"mysql":"9.7.2"}"#,
        )
        .unwrap();
        fs::create_dir(layout.data.join("mysql/mysql")).unwrap();
        fs::write(layout.data.join("database-ready"), "8.4").unwrap();
        let student = layout.data.join("mysql/student.ibd");
        fs::write(&student, b"original work").unwrap();
        let interrupted = layout.data.join("backups/interrupted.partial/mysql/mysql");
        fs::create_dir_all(&interrupted).unwrap();
        let unexpected = layout.data.join("mysql/unexpected-link");
        symlink(&student, &unexpected).unwrap();
        assert!(prepare_upgrade(&layout).is_err());
        assert!(!layout.data.join("database-upgrade.json").exists());
        assert_eq!(fs::read(&student).unwrap(), b"original work");
        assert!(fs::read_dir(layout.data.join("backups"))
            .unwrap()
            .all(|entry| entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "partial")));
        fs::remove_file(unexpected).unwrap();
        prepare_upgrade(&layout).unwrap();
        let journal: Upgrade =
            serde_json::from_slice(&fs::read(layout.data.join("database-upgrade.json")).unwrap())
                .unwrap();
        assert_ne!(
            journal.backup.extension().and_then(|value| value.to_str()),
            Some("partial")
        );
        assert_eq!(
            fs::read(journal.backup.join("mysql/student.ibd")).unwrap(),
            b"original work"
        );
        assert!(interrupted.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn upgrade_preserves_original_data_and_credentials_on_retry() {
        let root = std::env::temp_dir().join(format!(
            "fairway-upgrade-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        layout.initialize().unwrap();
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            br#"{"mysql":"9.7.2"}"#,
        )
        .unwrap();
        fs::create_dir(layout.data.join("mysql/mysql")).unwrap();
        fs::write(layout.data.join("mysql/student.ibd"), "original lessons").unwrap();
        fs::write(layout.data.join("database-ready"), "8.4").unwrap();
        fs::write(layout.data.join("credentials"), "original credentials").unwrap();
        prepare_upgrade(&layout).unwrap();
        let upgrade: Upgrade =
            serde_json::from_slice(&fs::read(layout.data.join("database-upgrade.json")).unwrap())
                .unwrap();
        fs::write(
            layout.data.join("mysql/student.ibd"),
            "changed by new server",
        )
        .unwrap();
        prepare_upgrade(&layout).unwrap();
        assert_eq!(
            fs::read_to_string(upgrade.backup.join("mysql/student.ibd")).unwrap(),
            "original lessons"
        );
        assert_eq!(
            fs::read_to_string(upgrade.backup.join("credentials")).unwrap(),
            "original credentials"
        );
        fs::write(layout.data.join("database-ready"), "9.7").unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            br#"{"mysql":"8.4.11"}"#,
        )
        .unwrap();
        assert!(prepare_upgrade(&layout)
            .unwrap_err()
            .contains("cannot be opened"));
        fs::remove_dir_all(root).unwrap();
    }
}
