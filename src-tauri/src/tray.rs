use crate::manager::Snapshot;

#[cfg(target_os = "macos")]
#[path = "tray_macos.rs"]
mod platform;
#[cfg(not(target_os = "macos"))]
#[path = "tray_windows.rs"]
mod platform;
pub(crate) use platform::NativeTray;

#[derive(Clone, Copy)]
enum Status {
    Stopped,
    Partial,
    Started,
}

impl Status {
    fn overall(snapshot: &Snapshot) -> Self {
        let running = ["php", "nginx", "mysql"]
            .into_iter()
            .filter(|id| {
                snapshot
                    .services
                    .iter()
                    .any(|service| service.id == *id && service.running)
            })
            .count();
        match running {
            0 => Self::Stopped,
            3 => Self::Started,
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
