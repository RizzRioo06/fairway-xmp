//! Preparation shared by the desktop updater and native release verification.
use crate::{
    config::{Layout, Result},
    manager::Manager,
};

/// This is called only after the entire download has passed signature checks.
pub fn prepare(manager: &mut Manager) -> Result<()> {
    if manager.layout.data.join("database-ready").exists() {
        let was_running = manager
            .snapshot()
            .services
            .iter()
            .any(|s| s.id == "mysql" && s.running);
        manager.start("mysql")?;
        if let Err(error) = manager.backup() {
            if !was_running {
                let _ = manager.stop("mysql");
            }
            return Err(format!(
                "The database backup failed. The update was not installed: {error}"
            ));
        }
    }
    manager.stop_all()?;
    clear_stopped_markers(&manager.layout)
}

/// NSIS checks these files before upgrading. Never conceal a live process by
/// deleting its marker; only remove stale markers after graceful shutdown.
pub fn clear_stopped_markers(layout: &Layout) -> Result<()> {
    for name in ["mysql.pid", "nginx.pid", "php.pid", "mail.pid"] {
        let path = layout.data.join(name);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.to_string()),
        };
        if !(name == "nginx.pid" && text.trim().is_empty()) {
            let pid = text
                .trim()
                .parse::<u32>()
                .map_err(|_| format!("Cannot verify {name}. The update was not installed."))?;
            if crate::process::pid_alive(pid) {
                return Err(format!(
                    "A service is still running ({name}). Wait and retry the update."
                ));
            }
        }
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_does_not_hide_a_live_process_or_an_invalid_pid() {
        let root = std::env::temp_dir().join(format!(
            "fairway-update-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut layout = Layout::new(root.join("runtime")).unwrap();
        layout.data = root.clone();
        let marker = root.join("mysql.pid");
        std::fs::write(&marker, std::process::id().to_string()).unwrap();
        assert!(clear_stopped_markers(&layout).is_err());
        assert!(marker.exists());
        std::fs::write(&marker, "invalid").unwrap();
        assert!(clear_stopped_markers(&layout).is_err());
        assert!(marker.exists());
        std::fs::remove_file(marker).unwrap();
        std::fs::write(root.join("nginx.pid"), "").unwrap();
        clear_stopped_markers(&layout).unwrap();
        assert!(!root.join("nginx.pid").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
