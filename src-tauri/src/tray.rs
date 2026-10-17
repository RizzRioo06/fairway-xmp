use crate::manager::Snapshot;

#[cfg(target_os = "macos")]
#[path = "tray_macos.rs"]
mod platform;
#[cfg(not(target_os = "macos"))]
#[path = "tray_windows.rs"]
mod platform;
pub(crate) use platform::NativeTray;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Stopped,
    Partial,
    Started,
}

impl Status {
    fn overall(snapshot: &Snapshot) -> Self {
        // A missing optional binary must not prevent "all started", but a
        // surviving mail process must always keep Stop all available.
        let mail_running = snapshot
            .services
            .iter()
            .any(|s| s.id == "mail" && s.running);
        let owned: &[&str] = if snapshot.mail_available || mail_running {
            &["php", "nginx", "mysql", "mail"]
        } else {
            &["php", "nginx", "mysql"]
        };
        let running = owned
            .iter()
            .copied()
            .filter(|id| {
                snapshot
                    .services
                    .iter()
                    .any(|service| service.id == *id && service.running)
            })
            .count();
        match running {
            0 => Self::Stopped,
            count if count == owned.len() => Self::Started,
            _ => Self::Partial,
        }
    }

    #[cfg(target_os = "macos")]
    fn service(ready: bool) -> Self {
        if ready {
            Self::Started
        } else {
            Self::Stopped
        }
    }

    #[cfg(target_os = "macos")]
    fn rgb(self) -> (f64, f64, f64) {
        match self {
            Self::Stopped => (0.81, 0.33, 0.33),
            Self::Partial => (0.56, 0.58, 0.60),
            Self::Started => (0.29, 0.63, 0.42),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(running: &[&str], mail_available: bool) -> Snapshot {
        let services = ["php", "nginx", "mysql", "phpmyadmin", "mail"].map(|id| {
            let active = running.contains(&id);
            serde_json::json!({ "id": id, "running": active, "ready": active, "port": 8800, "pid": null, "message": null })
        });
        serde_json::from_value(serde_json::json!({
            "ports": crate::config::Ports::default(), "services": services,
            "documentRoot": "", "projects": [], "domainsPending": false, "httpsTrusted": false,
            "os": "macos", "dataDir": "", "runtimeReady": true, "runtimeVersions": {},
            "mailAvailable": mail_available, "phpVersion": "8.5", "phpVersions": {},
            "pathInstalled": false, "cliPhp": null, "cliComposer": null,
            "pathPhp": null, "pathComposer": null, "version": "test"
        }))
        .unwrap()
    }

    #[test]
    fn mail_only_keeps_stop_all_enabled_and_missing_mail_keeps_start_all_enabled() {
        assert_eq!(Status::overall(&snapshot(&["mail"], true)), Status::Partial);
        assert_eq!(
            Status::overall(&snapshot(&["php", "nginx", "mysql"], true)),
            Status::Partial
        );
        assert_eq!(
            Status::overall(&snapshot(&["php", "nginx", "mysql", "mail"], true)),
            Status::Started
        );
        assert_eq!(Status::overall(&snapshot(&[], true)), Status::Stopped);
    }

    #[test]
    fn runtimes_without_mail_can_be_fully_started_but_running_mail_is_always_counted() {
        assert_eq!(
            Status::overall(&snapshot(&["php", "nginx", "mysql"], false)),
            Status::Started
        );
        assert_eq!(
            Status::overall(&snapshot(&["mail"], false)),
            Status::Partial
        );
        assert_eq!(Status::overall(&snapshot(&[], false)), Status::Stopped);
    }
}
