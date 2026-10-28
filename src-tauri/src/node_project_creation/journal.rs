//! Versioned, private durable jobs. No PID authority and no startup auto-recovery.
use super::{
    assets,
    filesystem::{failure, Dir, Identity, Receipt},
    generation::{JobStatus, Phase},
    CreationPlan, Error, ErrorCode, Result,
};
use crate::config::Layout;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Journal {
    pub schema_version: u32,
    pub revision: u64,
    pub plan: CreationPlan,
    pub parent_identity: String,
    pub stage_name: String,
    pub stage_identity: Option<Identity>,
    pub directories: BTreeMap<String, Identity>,
    pub files: BTreeMap<String, Receipt>,
    pub phase: Phase,
    pub project_id: Option<String>,
    pub error: Option<Error>,
    pub expected_registry_revision: Option<String>,
}
impl Journal {
    pub fn new(
        plan: CreationPlan,
        parent_identity: String,
        registry_revision: Option<String>,
    ) -> Self {
        Self {
            schema_version: 1,
            revision: 1,
            stage_name: format!(".fairway-node-{}", plan.id),
            plan,
            parent_identity,
            stage_identity: None,
            directories: BTreeMap::new(),
            files: BTreeMap::new(),
            phase: Phase::Prepared,
            project_id: None,
            error: None,
            expected_registry_revision: registry_revision,
        }
    }
    pub fn status(&self, active: bool) -> JobStatus {
        JobStatus {
            job_id: self.plan.id.clone(),
            revision: self.revision,
            phase: self.phase.clone(),
            destination: self.plan.destination.clone(),
            staging_path: Path::new(&self.plan.parent)
                .join(&self.stage_name)
                .to_string_lossy()
                .into(),
            project_id: self.project_id.clone(),
            error: self.error.clone(),
            active,
            recovery_required: !active
                && !matches!(self.phase, Phase::Completed | Phase::Cancelled),
        }
    }
    fn validate(&self, id: &str) -> Result<()> {
        let invalid = || {
            Error::new(
                ErrorCode::JournalUnavailable,
                "Invalid/stale creation journal; all files were preserved.",
            )
        };
        if self.schema_version != 1
            || self.plan.schema_version != 1
            || self.revision == 0
            || !super::hex(id, 32)
            || self.plan.id != id
            || self.stage_name != format!(".fairway-node-{id}")
        {
            return Err(invalid());
        }
        super::destination::validate_name(&self.plan.project_name)?;
        if !Path::new(&self.plan.parent).is_absolute()
            || Path::new(&self.plan.parent).join(&self.plan.project_name)
                != Path::new(&self.plan.destination)
        {
            return Err(invalid());
        }
        let mut unsigned = self.plan.clone();
        unsigned.digest.clear();
        if self.plan.digest
            != super::digest(
                &serde_json::to_vec(&(&unsigned, &self.parent_identity)).map_err(failure)?,
            )
        {
            return Err(invalid());
        }
        // Recovery uses the review manifest persisted with the job, not today's
        // template sources. A later asset update must not erase recovery authority.
        let intended: std::collections::BTreeSet<_> =
            self.plan.template.intended_files.iter().collect();
        let reviewed: std::collections::BTreeSet<_> = self.plan.files.keys().collect();
        let total = self
            .plan
            .files
            .values()
            .try_fold(0u64, |total, file| total.checked_add(file.bytes));
        if intended != reviewed
            || reviewed.is_empty()
            || reviewed.len() > 128
            || total.is_none_or(|bytes| bytes > 2 * 1024 * 1024)
        {
            return Err(invalid());
        }
        for (name, file) in &self.plan.files {
            assets::validate_entry(name)?;
            if file.bytes > 256 * 1024 || !super::hex(&file.sha256, 64) {
                return Err(invalid());
            }
        }
        for (name, receipt) in &self.files {
            assets::validate_entry(name)?;
            let expected = self.plan.files.get(name).ok_or_else(invalid)?;
            if receipt.sha256 != expected.sha256
                || receipt.bytes != expected.bytes
                || receipt.mode != 0o600
            {
                return Err(invalid());
            }
        }
        let expected = super::filesystem::directories(self.plan.files.keys().cloned());
        if self.directories.keys().any(|name| !expected.contains(name)) {
            return Err(invalid());
        }
        Ok(())
    }
}
pub(super) struct Store {
    pub dir: Dir,
    path: PathBuf,
}
impl Store {
    pub fn open(layout: &Layout, create: bool) -> Result<Option<Self>> {
        let data_parent = layout
            .data
            .parent()
            .ok_or_else(|| failure("Missing application data parent"))?;
        let canonical = match std::fs::canonicalize(data_parent) {
            Ok(path) => path,
            Err(error) if !create && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None)
            }
            Err(error) => return Err(failure(error)),
        };
        let parent = Dir::absolute(&canonical)?;
        let name = layout
            .data
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| failure("Invalid application data name"))?;
        let data = if parent.entry(name)?.is_some() {
            parent.child(name)?
        } else if create {
            parent.assert_path(&canonical)?;
            parent.make(name)?
        } else {
            return Ok(None);
        };
        let data_path = canonical.join(name);
        data.assert_path(&data_path)?;
        let dir = if data.entry("node-project-creation")?.is_some() {
            let dir = data.child("node-project-creation")?;
            dir.private()?;
            dir
        } else if create {
            data.assert_path(&data_path)?;
            data.make("node-project-creation")?
        } else {
            return Ok(None);
        };
        let path = data_path.join("node-project-creation");
        dir.assert_path(&path)?;
        Ok(Some(Self { dir, path }))
    }
    pub fn lock(&self) -> Result<File> {
        self.dir.assert_path(&self.path)?;
        self.dir.lock("creation.lock")
    }
    pub fn claim(&self, journal: &Journal) -> Result<()> {
        self.dir.assert_path(&self.path)?;
        let name = format!("{}.json", journal.plan.id);
        if self.dir.entry(&name)?.is_some() {
            return Err(Error::new(
                ErrorCode::PlanConsumed,
                "This plan already has a durable creation journal.",
            ));
        }
        if self
            .dir
            .names()?
            .iter()
            .filter(|name| name.ends_with(".json"))
            .count()
            >= 256
        {
            return Err(Error::new(ErrorCode::PlanLimit,"Creation history is full. Preserve/review existing journals before creating more projects."));
        }
        self.dir
            .create(&name, &serde_json::to_vec(journal).map_err(failure)?)?;
        Ok(())
    }
    pub fn save(&self, journal: &mut Journal) -> Result<()> {
        self.dir.assert_path(&self.path)?;
        journal.revision = journal
            .revision
            .checked_add(1)
            .ok_or_else(|| failure("Journal revision overflow"))?;
        self.dir.atomic(
            &format!("{}.json", journal.plan.id),
            &serde_json::to_vec(journal).map_err(failure)?,
        )
    }
    pub fn load(&self, id: &str) -> Result<Journal> {
        if !super::hex(id, 32) {
            return Err(Error::new(ErrorCode::InvalidRequest, "Invalid job ID."));
        }
        self.dir.assert_path(&self.path)?;
        let (bytes, receipt) = self.dir.read(&format!("{id}.json"), 2 * 1024 * 1024)?;
        if receipt.mode != 0o600 {
            return Err(failure("Journal permissions changed"));
        }
        let journal: Journal = serde_json::from_slice(&bytes).map_err(|_| {
            Error::new(
                ErrorCode::JournalUnavailable,
                "Creation journal is malformed; it was preserved.",
            )
        })?;
        journal.validate(id)?;
        Ok(journal)
    }
    pub fn list(&self) -> Result<(Vec<Journal>, Vec<String>)> {
        let mut jobs = Vec::new();
        let mut errors = Vec::new();
        for name in self.dir.names()? {
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            match self.load(id) {
                Ok(job) => jobs.push(job),
                Err(error) => errors.push(format!("{name}: {}", error.message)),
            }
            if jobs.len() + errors.len() >= 256 {
                break;
            }
        }
        Ok((jobs, errors))
    }
}
