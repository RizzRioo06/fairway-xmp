use super::*;
use crate::{
    node_project_creation::{
        assets, destination,
        filesystem::{self, Dir},
        journal::{Journal, Store},
        ErrorCode, NodeProjectCreation, PlanReference, Result,
    },
    node_projects::{self, NodeProjects, ProjectConfig},
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
struct Active<'a>(&'a NodeProjectCreation);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}
fn busy() -> Error {
    Error::new(
        ErrorCode::GenerationBusy,
        "A local creation/recovery operation is already in progress.",
    )
}
fn parent(journal: &Journal) -> Result<Dir> {
    let dir = Dir::absolute(Path::new(&journal.plan.parent))?;
    if dir.identity_text()? != journal.parent_identity {
        return Err(Error::new(
            ErrorCode::RecoveryUnsafe,
            "The destination parent was replaced. Files were preserved.",
        ));
    }
    Ok(dir)
}
fn stage(parent: &Dir, journal: &Journal, name: &str) -> Result<Dir> {
    let dir = parent.child(name)?;
    dir.private()?;
    if Some(dir.id()?) != journal.stage_identity {
        return Err(Error::new(
            ErrorCode::RecoveryUnsafe,
            "The job directory identity changed. Files were preserved.",
        ));
    }
    parent.verify_child(name, &dir)?;
    Ok(dir)
}
fn discard(store: &Store, journal: &mut Journal) -> Result<()> {
    let parent = parent(journal)?;
    parent.assert_path(Path::new(&journal.plan.parent))?;
    if let Some((id, _)) = parent.entry(&journal.plan.project_name)? {
        if Some(id) == journal.stage_identity {
            return Err(Error::new(
                ErrorCode::CancellationTooLate,
                "This project was published. It will never be deleted by staging recovery.",
            ));
        }
    }
    match parent.entry(&journal.stage_name)? {
        None if journal.stage_identity.is_none() => (),
        None => {
            return Err(Error::new(
                ErrorCode::RecoveryUnsafe,
                "The staging directory is missing or moved; nothing was deleted.",
            ))
        }
        Some(_) => {
            let dir = stage(&parent, journal, &journal.stage_name)?;
            if !journal.files.contains_key(".fairway-template.json") {
                return Err(Error::new(ErrorCode::RecoveryUnsafe,"No durable ownership marker identifies this staging directory. It was preserved."));
            }
            filesystem::clean(&dir, &journal.files, &journal.directories)?;
            parent.assert_path(Path::new(&journal.plan.parent))?;
            parent.verify_child(&journal.stage_name, &dir)?;
            parent.unlink_dir(&journal.stage_name)?;
        }
    }
    journal.phase = Phase::Cancelled;
    journal.error = None;
    store.save(journal)
}
fn register(
    service: &NodeProjectCreation,
    nodes: &NodeProjects,
    store: &Store,
    journal: &mut Journal,
) -> Result<()> {
    let parent = parent(journal)?;
    parent.assert_path(Path::new(&journal.plan.parent))?;
    let root = stage(&parent, journal, &journal.plan.project_name)?;
    if journal.files.keys().collect::<Vec<_>>() != journal.plan.files.keys().collect::<Vec<_>>() {
        return Err(Error::new(
            ErrorCode::RecoveryUnsafe,
            "An incomplete project cannot be registered by publication recovery.",
        ));
    }
    filesystem::verify_tree(&root, &journal.files, &journal.directories)?;
    let runtime = node_projects::ProjectRuntime {
        node_path: journal.plan.tools.node_path.clone(),
        version: journal.plan.tools.node_version.clone(),
    };
    let prepared = (service.prepare)(
        service.layout()?,
        runtime.clone(),
        Path::new(&journal.plan.parent),
        Path::new(&journal.plan.destination),
        &journal.plan.template.node_range,
        &journal.plan.template.package_manager.version_range,
    )?;
    if prepared.runtime_revision != journal.plan.tools.revision
        || prepared.npm.version != journal.plan.tools.npm_version
        || prepared.npm.cli_path != journal.plan.tools.npm_cli_path
    {
        return Err(Error::new(ErrorCode::RuntimeChanged,"The selected tools changed. Published files were retained; review/import them explicitly."));
    }
    parent.assert_path(Path::new(&journal.plan.parent))?;
    parent.verify_child(&journal.plan.project_name, &root)?;
    filesystem::verify_tree(&root, &journal.files, &journal.directories)?;
    let inspection = node_projects::inspect_generated_manifest(&journal.plan.destination)?;
    let config = ProjectConfig {
        id: journal.plan.id.clone(),
        name: journal.plan.project_name.clone(),
        root: journal.plan.destination.clone(),
        script: "dev".into(),
        runtime,
        open_url: None,
    };
    let saved=nodes.register_generated(config,journal.expected_registry_revision.as_deref(),&inspection.manifest_revision)
        .map_err(|error|Error::new(ErrorCode::RegistrationFailed,format!("Published project retained. Retry Registration after reviewing the registry: {}",error.message)))?;
    parent.assert_path(Path::new(&journal.plan.parent))?;
    parent.verify_child(&journal.plan.project_name, &root)?;
    journal.project_id = Some(saved.project.id);
    journal.phase = Phase::Completed;
    journal.error = None;
    store.save(journal)
}
impl NodeProjectCreation {
    pub fn generate(&self, nodes: &NodeProjects, request: GenerateRequest) -> Result<JobStatus> {
        self.generate_inner(nodes, request, |_, _| Ok(()))
    }
    pub(in crate::node_project_creation) fn generate_inner(
        &self,
        nodes: &NodeProjects,
        request: GenerateRequest,
        mut hook: impl FnMut(&str, &mut Journal) -> Result<()>,
    ) -> Result<JobStatus> {
        self.layout()?;
        if !request.confirmed {
            return Err(Error::new(
                ErrorCode::ConfirmationRequired,
                "Local project generation requires explicit confirmation.",
            ));
        }
        let _operation = self.operations.try_lock().map_err(|_| busy())?;
        let plan = self.validate_plan(PlanReference {
            plan_id: request.plan_id.clone(),
            digest: request.digest.clone(),
        })?;
        let files = assets::render(&plan)?;
        if assets::describe(&files) != plan.files {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "Template content changed. Create/review a fresh plan.",
            ));
        }
        let location = destination::validate(&plan.parent, &plan.project_name)?;
        let parent = Dir::absolute(Path::new(&plan.parent))?;
        if parent.identity_text()? != location.identity {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "The destination parent changed before mutation.",
            ));
        }
        // Recheck native authority immediately before the first durable write.
        let mut plans = self.plans.lock().map_err(|_| busy())?;
        let stored = plans
            .values_mut()
            .find(|stored| stored.plan.id == plan.id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::PlanUnavailable,
                    "The review plan is unavailable.",
                )
            })?;
        if stored.consumed
            || stored.plan.digest != request.digest
            || Instant::now() >= stored.deadline
            || stored.location != location
        {
            return Err(Error::new(
                ErrorCode::PlanStale,
                "Plan authority changed before generation.",
            ));
        }
        parent.assert_path(Path::new(&plan.parent))?;
        if parent.entry(&plan.project_name)?.is_some() {
            return Err(Error::new(
                ErrorCode::DestinationExists,
                "The destination became occupied.",
            ));
        }
        let store = Store::open(self.layout()?, true)?.expect("create store");
        let _lock = store.lock()?;
        let mut journal = Journal::new(plan, location.identity.clone(), nodes.snapshot().revision);
        store.claim(&journal)?;
        stored.consumed = true;
        let deadline = stored.deadline;
        let request = stored.request.clone();
        drop(plans);
        let cancelled = Arc::new(AtomicBool::new(false));
        *self.active.lock().map_err(|_| busy())? = Some(Control {
            id: journal.plan.id.clone(),
            cancelled: cancelled.clone(),
            publishing: false,
        });
        let _active = Active(self);
        let guard = |journal: &Journal| -> Result<()> {
            if cancelled.load(Ordering::SeqCst) {
                return Err(Error::new(
                    ErrorCode::GenerationCancelled,
                    "Generation was cancelled before publication.",
                ));
            }
            if Instant::now() >= deadline {
                return Err(Error::new(
                    ErrorCode::PlanExpired,
                    "The review plan expired before publication.",
                ));
            }
            parent.assert_path(Path::new(&journal.plan.parent))?;
            if parent.entry(&journal.plan.project_name)?.is_some() {
                return Err(Error::new(
                    ErrorCode::DestinationExists,
                    "The destination became occupied.",
                ));
            }
            Ok(())
        };
        let result = (|| -> Result<()> {
            hook("prepared", &mut journal)?;
            guard(&journal)?;
            let root = parent.make(&journal.stage_name)?;
            journal.stage_identity = Some(root.id()?);
            // A random creation ID in a receipted marker distinguishes an owned
            // stage across crashes; inode numbers alone can eventually be reused.
            parent.assert_path(Path::new(&journal.plan.parent))?;
            parent.verify_child(&journal.stage_name, &root)?;
            guard(&journal)?;
            let marker = root.create(".fairway-template.json", &files[".fairway-template.json"])?;
            journal
                .files
                .insert(".fairway-template.json".into(), marker);
            journal.phase = Phase::Staging;
            store.save(&mut journal)?;
            hook("staging", &mut journal)?;
            for name in filesystem::directories(files.keys().cloned()) {
                guard(&journal)?;
                parent.verify_child(&journal.stage_name, &root)?;
                let (dir, leaf) = filesystem::parent_of(&root, &name, &journal.directories)?;
                let made = dir.make(&leaf)?;
                journal.directories.insert(name, made.id()?);
                store.save(&mut journal)?;
            }
            for (name, bytes) in &files {
                if name == ".fairway-template.json" {
                    continue;
                }
                guard(&journal)?;
                parent.verify_child(&journal.stage_name, &root)?;
                let (dir, leaf) = filesystem::parent_of(&root, name, &journal.directories)?;
                let receipt = dir.create(&leaf, bytes)?;
                journal.files.insert(name.clone(), receipt);
                store.save(&mut journal)?;
                hook("file", &mut journal)?;
            }
            filesystem::verify_tree(&root, &journal.files, &journal.directories)?;
            journal.phase = Phase::Ready;
            store.save(&mut journal)?;
            hook("ready", &mut journal)?;
            // Fresh tool validation happens while the destination is still absent.
            if self.tools(&request, &journal.plan.template, &location)? != journal.plan.tools {
                return Err(Error::new(
                    ErrorCode::RuntimeChanged,
                    "Selected tools changed during generation.",
                ));
            }
            hook("beforePublish", &mut journal)?;
            guard(&journal)?;
            {
                let mut active = self.active.lock().map_err(|_| busy())?;
                if cancelled.load(Ordering::SeqCst) {
                    return Err(Error::new(
                        ErrorCode::GenerationCancelled,
                        "Generation was cancelled.",
                    ));
                }
                active.as_mut().ok_or_else(busy)?.publishing = true;
            }
            journal.phase = Phase::Publishing;
            store.save(&mut journal)?;
            hook("atPublish", &mut journal)?;
            guard(&journal)?;
            parent.verify_child(&journal.stage_name, &root)?;
            filesystem::verify_tree(&root, &journal.files, &journal.directories)?;
            parent.no_replace(&journal.stage_name, &journal.plan.project_name)?;
            hook("afterPublish", &mut journal)?;
            parent.assert_path(Path::new(&journal.plan.parent))?;
            parent.verify_child(&journal.plan.project_name, &root)?;
            journal.phase = Phase::NeedsRegistration;
            store.save(&mut journal)?;
            hook("beforeRegistration", &mut journal)?;
            register(self, nodes, &store, &mut journal)
        })();
        if let Err(error) = result {
            if error.code == ErrorCode::GenerationCancelled {
                if let Err(cleanup) = discard(&store, &mut journal) {
                    journal.phase = Phase::Preserved;
                    journal.error = Some(cleanup);
                    store.save(&mut journal)?;
                }
            } else {
                // Publication is irreversible here: no rollback/deletion of a project.
                let published = parent
                    .entry(&journal.plan.project_name)
                    .ok()
                    .flatten()
                    .is_some_and(|(id, _)| Some(id) == journal.stage_identity);
                journal.phase = if published {
                    Phase::NeedsRegistration
                } else {
                    Phase::Preserved
                };
                journal.error = Some(error);
                store.save(&mut journal)?;
            }
        }
        Ok(journal.status(false))
    }
    pub fn cancel_generation(&self, request: CancelRequest) -> Result<()> {
        self.layout()?;
        let active = self.active.lock().map_err(|_| busy())?;
        let job = active
            .as_ref()
            .filter(|job| job.id == request.job_id)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::JobUnavailable,
                    "No active generation matches this job ID.",
                )
            })?;
        if job.publishing {
            return Err(Error::new(
                ErrorCode::CancellationTooLate,
                "Publication/registration has begun; the generated folder will be retained.",
            ));
        }
        job.cancelled.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub fn recover(&self, nodes: &NodeProjects, request: RecoverRequest) -> Result<JobStatus> {
        self.layout()?;
        if !request.confirmed {
            return Err(Error::new(
                ErrorCode::ConfirmationRequired,
                "Recovery requires explicit confirmation.",
            ));
        }
        let _operation = self.operations.try_lock().map_err(|_| busy())?;
        let store = Store::open(self.layout()?, false)?
            .ok_or_else(|| Error::new(ErrorCode::JobUnavailable, "No creation journal exists."))?;
        let _lock = store.lock()?;
        let mut journal = store.load(&request.job_id)?;
        if request.expected_revision != journal.revision {
            return Err(Error::new(
                ErrorCode::StaleJob,
                "The journal changed; reload before retrying recovery.",
            ));
        }
        if matches!(journal.phase, Phase::Completed | Phase::Cancelled) {
            return Err(Error::new(
                ErrorCode::StaleJob,
                "This job is already terminal. No mutation was repeated.",
            ));
        }
        match request.action {
            RecoveryAction::DiscardStaging => {
                if let Err(error) = discard(&store, &mut journal) {
                    journal.phase = Phase::Preserved;
                    journal.error = Some(error);
                    store.save(&mut journal)?;
                }
            }
            RecoveryAction::RetryRegistration => {
                let parent = parent(&journal)?;
                if parent.entry(&journal.stage_name)?.is_some() {
                    return Err(Error::new(ErrorCode::RecoveryUnsafe,"Unpublished staging remains. It cannot be registered or automatically published during recovery."));
                }
                let root = stage(&parent, &journal, &journal.plan.project_name)?;
                filesystem::verify_tree(&root, &journal.files, &journal.directories)?;
                journal.expected_registry_revision = nodes.snapshot().revision;
                journal.phase = Phase::NeedsRegistration;
                store.save(&mut journal)?;
                if let Err(error) = register(self, nodes, &store, &mut journal) {
                    journal.phase = Phase::NeedsRegistration;
                    journal.error = Some(error);
                    store.save(&mut journal)?;
                }
            }
        }
        Ok(journal.status(false))
    }
    pub fn creation_jobs(&self) -> JobsSnapshot {
        let mut snapshot = JobsSnapshot {
            supported: self.supported,
            jobs: Vec::new(),
            errors: Vec::new(),
        };
        let result = (|| -> Result<()> {
            let Some(store) = Store::open(self.layout()?, false)? else {
                return Ok(());
            };
            let (jobs, errors) = store.list()?;
            snapshot.errors = errors;
            let active = self.active.lock().map_err(|_| busy())?;
            snapshot.jobs = jobs
                .into_iter()
                .map(|job| {
                    let running = active
                        .as_ref()
                        .is_some_and(|active| active.id == job.plan.id);
                    job.status(running)
                })
                .collect();
            Ok(())
        })();
        if let Err(error) = result {
            snapshot.errors.push(error.message);
        }
        snapshot
    }
}
