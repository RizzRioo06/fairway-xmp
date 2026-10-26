//! One cancellable Node installation, independent of the service-manager lock.
use crate::config::Result;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Serialize)]
pub struct Installation {
    pub id: String,
    pub version: String,
    pub phase: &'static str,
}

struct Active {
    progress: Installation,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone, Default)]
pub struct Installations(Arc<Mutex<Option<Active>>>);

impl Installations {
    pub fn start(&self, id: &str, version: &str) -> Result<InstallationGuard> {
        if id.is_empty() || id.len() > 100 || !super::valid_version(version) {
            return Err("Invalid Node installation request.".into());
        }
        let mut active = self
            .0
            .lock()
            .map_err(|_| "Node installation is unavailable.")?;
        if active.is_some() {
            return Err("A Node installation is already running.".into());
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        *active = Some(Active {
            progress: Installation {
                id: id.into(),
                version: version.into(),
                phase: "installing",
            },
            cancelled: cancelled.clone(),
        });
        Ok(InstallationGuard {
            jobs: self.clone(),
            id: id.into(),
            cancelled,
        })
    }

    pub fn current(&self) -> Option<Installation> {
        self.0
            .lock()
            .ok()?
            .as_ref()
            .map(|active| active.progress.clone())
    }

    pub fn cancel(&self, id: &str) {
        if let Ok(active) = self.0.lock() {
            if let Some(active) = active
                .as_ref()
                .filter(|active| active.progress.id == id && active.progress.phase == "installing")
            {
                active.cancelled.store(true, Ordering::SeqCst);
            }
        }
    }
}

pub struct InstallationGuard {
    jobs: Installations,
    id: String,
    cancelled: Arc<AtomicBool>,
}

impl InstallationGuard {
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Serialize cancellation with the commit boundary. Once activation starts,
    /// its transaction finishes or rolls back before releasing the operation.
    pub fn activating(&self) -> Result<()> {
        let mut active = self
            .jobs
            .0
            .lock()
            .map_err(|_| "Node installation is unavailable.")?;
        if self.cancelled() {
            return Err("Command cancelled.".into());
        }
        if let Some(active) = active
            .as_mut()
            .filter(|active| active.progress.id == self.id)
        {
            active.progress.phase = "activating";
        }
        Ok(())
    }
}

impl Drop for InstallationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.jobs.0.lock() {
            if active
                .as_ref()
                .is_some_and(|active| active.progress.id == self.id)
            {
                *active = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_scoped_and_cleanup_allows_retry() {
        let jobs = Installations::default();
        let first = jobs.start("first", "v24.1.0").unwrap();
        assert!(jobs.start("second", "v24.1.0").is_err());
        jobs.cancel("other");
        assert!(!first.cancelled());
        jobs.cancel("first");
        assert!(first.cancelled());
        assert!(first.activating().is_err());
        drop(first);
        assert!(jobs.current().is_none());
        let retry = jobs.start("retry", "v24.1.0").unwrap();
        jobs.cancel("first");
        assert!(!retry.cancelled());
        retry.activating().unwrap();
        jobs.cancel("retry");
        assert!(!retry.cancelled());
        assert_eq!(jobs.current().unwrap().phase, "activating");
    }
}
