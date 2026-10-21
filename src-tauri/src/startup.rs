//! Keep startup failures visible without replacing invalid settings or credentials.
use crate::{
    config::{Layout, Result},
    integration,
    manager::Manager,
};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Status {
    pub blocking: bool,
    pub errors: Vec<String>,
    pub data_dir: Option<String>,
}

pub(crate) struct Startup {
    runtime: PathBuf,
    pub layout: Option<Layout>,
    pub manager: Option<Manager>,
    errors: Vec<String>,
    persistent_errors: Vec<String>,
}

impl Startup {
    pub fn new(runtime: PathBuf) -> Self {
        let mut startup = Self {
            runtime,
            layout: None,
            manager: None,
            errors: Vec::new(),
            persistent_errors: Vec::new(),
        };
        startup.retry();
        startup
    }

    pub fn retry(&mut self) -> Status {
        self.errors.clear();
        if self.layout.is_none() {
            match Layout::new(self.runtime.clone()) {
                Ok(layout) => self.layout = Some(layout),
                Err(error) => self.errors.push(error),
            }
        }
        if let Some(layout) = &self.layout {
            if self.manager.is_none() {
                match Manager::new(layout.clone()) {
                    Ok(manager) => self.manager = Some(manager),
                    Err(error) => self.errors.push(error),
                }
            }
            if self.manager.is_some() {
                if let Err(error) = integration::refresh(layout) {
                    self.errors.push(format!(
                        "Command-line tools could not be refreshed: {error}"
                    ));
                }
                if let Err(error) = crate::config::atomic_write(
                    &layout.data.join("app.pid"),
                    std::process::id().to_string().as_bytes(),
                ) {
                    self.errors.push(format!(
                        "The application marker could not be saved: {error}"
                    ));
                }
            }
        }
        self.status()
    }

    pub fn status(&self) -> Status {
        Status {
            blocking: self.manager.is_none(),
            errors: self
                .errors
                .iter()
                .chain(&self.persistent_errors)
                .cloned()
                .collect(),
            data_dir: self
                .layout
                .as_ref()
                .map(|layout| layout.data.to_string_lossy().into_owned()),
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.persistent_errors.push(error);
    }

    pub fn manager(&mut self) -> Result<&mut Manager> {
        self.manager
            .as_mut()
            .ok_or_else(|| format!("Startup needs attention. {}", self.errors.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> (Startup, PathBuf) {
        let root = std::env::temp_dir().join(format!("fairway-startup-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        layout.initialize().unwrap();
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            br#"{"php":"8.5.11","mysql":"9.7.2"}"#,
        )
        .unwrap();
        (
            Startup {
                runtime: layout.runtime.clone(),
                layout: Some(layout),
                manager: None,
                errors: vec![],
                persistent_errors: vec![],
            },
            root,
        )
    }

    #[test]
    fn invalid_settings_are_reported_and_preserved_until_external_repair() {
        for invalid in ["not JSON", r#"{"website":8800,"newerField":true}"#] {
            let (mut startup, root) = fixture();
            let layout = startup.layout.clone().unwrap();
            fs::write(layout.data.join("settings.json"), invalid).unwrap();
            let credentials = "b".repeat(48);
            fs::write(layout.data.join("credentials"), &credentials).unwrap();
            let status = startup.retry();
            assert!(status.blocking && !status.errors.is_empty());
            assert!(startup.manager().is_err());
            assert_eq!(
                fs::read_to_string(layout.data.join("settings.json")).unwrap(),
                invalid
            );
            layout.save_ports(&crate::config::Ports::default()).unwrap();
            assert!(!startup.retry().blocking);
            assert_eq!(
                fs::read_to_string(layout.data.join("credentials")).unwrap(),
                credentials
            );
            assert!(startup
                .manager()
                .unwrap()
                .snapshot()
                .services
                .iter()
                .all(|service| !service.running));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn unavailable_php_selection_and_certificate_failure_are_recoverable() {
        let (mut startup, root) = fixture();
        let layout = startup.layout.clone().unwrap();
        fs::write(layout.data.join("php-version"), "7.4").unwrap();
        assert!(startup.retry().blocking);
        assert_eq!(
            fs::read_to_string(layout.data.join("php-version")).unwrap(),
            "7.4"
        );
        fs::write(layout.data.join("php-version"), "8.5").unwrap();
        fs::create_dir(layout.data.join("tls")).unwrap();
        fs::write(layout.data.join("tls/root-ca.pem"), "preserved certificate").unwrap();
        layout
            .save_projects(&[crate::config::ProjectConfig {
                name: "Shop".into(),
                domain: "shop".into(),
                root: layout.www.clone(),
                https: true,
                index_fallback: false,
            }])
            .unwrap();
        let status = startup.retry();
        assert!(status.blocking && status.errors[0].contains("incomplete"));
        assert_eq!(
            fs::read_to_string(layout.data.join("tls/root-ca.pem")).unwrap(),
            "preserved certificate"
        );
        layout.save_projects(&[]).unwrap();
        assert!(!startup.retry().blocking);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cli_refresh_failure_keeps_manager_available_and_can_be_retried() {
        let (mut startup, root) = fixture();
        let layout = startup.layout.clone().unwrap();
        fs::write(layout.data.join("cli-installed"), "enabled").unwrap();
        let launcher = layout
            .bin
            .join(if cfg!(windows) { "php.cmd" } else { "php" });
        fs::create_dir(&launcher).unwrap();
        let status = startup.retry();
        assert!(!status.blocking && status.errors[0].contains("could not be refreshed"));
        assert!(startup.manager().is_ok());
        fs::remove_dir(launcher).unwrap();
        assert!(startup.retry().errors.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
