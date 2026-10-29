use super::*;
use crate::node_project_creation::{
    filesystem::{self, Dir, Identity},
    generation::{
        native::{discard, parent, register, stage},
        GenerateRequest, Phase,
    },
    journal::{Journal, Store},
    NodeProjectCreation,
};
use std::{
    fs::File,
    os::{
        fd::AsRawFd,
        unix::{fs::MetadataExt, process::CommandExt},
    },
    process::Command,
    thread,
};
fn io(error: impl std::fmt::Display) -> Error {
    Error::new(
        ErrorCode::InstallationFailed,
        format!("Installation failed; workspace retained: {error}"),
    )
}
fn node_error(error: Error) -> node_projects::Error {
    node_projects::Error {
        code: node_projects::ErrorCode::ExecutionFailed,
        message: error.message,
    }
}
fn find(service: &NodeProjectCreation, id: &str) -> Result<(Store, Journal)> {
    if !crate::node_project_creation::hex(id, 32) {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "Invalid installation ID.",
        ));
    }
    let store = Store::open(service.layout()?, false)?
        .ok_or_else(|| Error::new(ErrorCode::JobUnavailable, "No installation journal exists."))?;
    let (jobs, _) = store.list()?;
    let journal = jobs
        .into_iter()
        .find(|journal| {
            journal
                .installation
                .as_ref()
                .is_some_and(|status| status.job_id == id)
        })
        .ok_or_else(|| {
            Error::new(
                ErrorCode::JobUnavailable,
                "The installation journal is unavailable.",
            )
        })?;
    Ok((store, journal))
}
pub(super) fn dependency_snapshot(root: &Dir) -> Result<Option<DependencyReceipt>> {
    let Some((identity, kind)) = root.entry("node_modules")? else {
        return Ok(None);
    };
    if kind & libc::S_IFMT as u32 != libc::S_IFDIR as u32 {
        return Err(io(
            "node_modules must be an owned directory, never a root symlink",
        ));
    }
    let dir = root.child("node_modules")?;
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    let mut entries = 0u64;
    let mut bytes = 0u64;
    fn walk(
        base: &Dir,
        dir: &Dir,
        prefix: &str,
        hash: &mut ring::digest::Context,
        entries: &mut u64,
        bytes: &mut u64,
        depth: usize,
    ) -> Result<()> {
        let metadata = dir.file()?.metadata().map_err(io)?;
        if metadata.uid() != unsafe { libc::geteuid() } || depth > 64 {
            return Err(io("Unsafe dependency directory ownership/depth"));
        }
        hash.update(&serde_json::to_vec(&(prefix, dir.id()?, metadata.mode())).map_err(io)?);
        for name in dir.names_bounded(8192)? {
            *entries += 1;
            if *entries > 100_000 {
                return Err(io("Too many installed dependency entries"));
            }
            let path = format!("{prefix}{name}");
            if path.len() > 4096 {
                return Err(io("Dependency path exceeds its bound"));
            }
            let (id, mode) = dir
                .entry(&name)?
                .ok_or_else(|| io("Dependency entry disappeared"))?;
            if mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32 {
                walk(
                    base,
                    &dir.child(&name)?,
                    &format!("{path}/"),
                    hash,
                    entries,
                    bytes,
                    depth + 1,
                )?;
            } else if mode & libc::S_IFMT as u32 == libc::S_IFREG as u32 {
                let (_, receipt) = dir.read(&name, 64 * 1024 * 1024)?;
                *bytes = bytes
                    .checked_add(receipt.bytes)
                    .ok_or_else(|| io("Dependency size overflow"))?;
                if *bytes > 1024 * 1024 * 1024 {
                    return Err(io("Dependencies exceed 1 GiB verification bound"));
                }
                hash.update(&serde_json::to_vec(&(&path, receipt)).map_err(io)?);
            } else if mode & libc::S_IFMT as u32 == libc::S_IFLNK as u32 {
                let target = dir.link_text(&name)?;
                if target.starts_with('/')
                    || target.contains('\\')
                    || target.chars().any(char::is_control)
                {
                    return Err(io("Unsafe dependency symlink"));
                }
                let mut parts: Vec<_> = prefix.split('/').filter(|part| !part.is_empty()).collect();
                for part in target.split('/') {
                    match part {
                        "" | "." => (),
                        ".." => {
                            if parts.pop().is_none() {
                                return Err(io("Dependency symlink escapes node_modules"));
                            }
                        }
                        _ => parts.push(part),
                    }
                }
                if prefix != ".bin/" || parts.len() < 2 {
                    return Err(io("Only reviewed npm .bin links are supported"));
                }
                let mut target_dir = base.child(parts[0])?;
                for part in &parts[1..parts.len() - 1] {
                    target_dir = target_dir.child(part)?;
                }
                target_dir.read(parts[parts.len() - 1], 64 * 1024 * 1024)?;
                hash.update(&serde_json::to_vec(&(path, id, target)).map_err(io)?);
            } else {
                return Err(io("Special dependency files are not supported"));
            }
        }
        Ok(())
    }
    walk(&dir, &dir, "", &mut hash, &mut entries, &mut bytes, 0)?;
    Ok(Some(DependencyReceipt {
        dev: identity.dev,
        ino: identity.ino,
        sha256: hash
            .finish()
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        entries,
        bytes,
    }))
}
pub(in crate::node_project_creation) fn verify_project(
    root: &Dir,
    journal: &Journal,
) -> Result<()> {
    let dependency = journal.dependencies.as_ref().map(|receipt| Identity {
        dev: receipt.dev,
        ino: receipt.ino,
    });
    filesystem::verify_tree_with_dependencies(
        root,
        &journal.files,
        &journal.directories,
        dependency.as_ref(),
    )?;
    if dependency_snapshot(root)? != journal.dependencies {
        return Err(io("Installed dependencies changed; data was preserved"));
    }
    Ok(())
}
fn registry_allowed(value: &str) -> bool {
    if value == "https://registry.npmjs.org/" {
        return true;
    }
    #[cfg(all(debug_assertions, feature = "node-install-fixtures"))]
    if let Some(port) = value
        .strip_prefix("http://127.0.0.1:")
        .and_then(|value| value.strip_suffix('/'))
    {
        return port.parse::<u16>().is_ok_and(|port| port != 0);
    }
    false
}
pub(crate) fn runner_lock(data: &Path, creation_id: &str) -> node_projects::Result<File> {
    let layout = crate::config::Layout::at(data.into(), data.into(), data.into());
    let store = Store::open(&layout, false)
        .map_err(node_error)?
        .ok_or_else(|| node_error(io("Missing installation journal")))?;
    let journal = store.load(creation_id).map_err(node_error)?;
    let parent = parent(&journal).map_err(node_error)?;
    let root = stage(&parent, &journal, &journal.stage_name).map_err(node_error)?;
    root.lock_existing(".fairway-install.lock")
        .map_err(node_error)
}
pub(crate) fn runner_command(
    layout: &crate::config::Layout,
    creation_id: &str,
    job_id: &str,
    run_id: &str,
) -> node_projects::Result<Command> {
    (|| -> Result<Command> {
        let store = Store::open(layout, false)?.ok_or_else(|| io("Missing creation journal"))?;
        let journal = store.load(creation_id)?;
        let status = journal
            .installation
            .as_ref()
            .ok_or_else(|| io("No installation authorization"))?;
        if status.job_id != job_id
            || status.run_id != run_id
            || status.state != InstallState::Starting
            || !registry_allowed(&status.registry)
        {
            return Err(io("Installation authorization is stale or invalid"));
        }
        let parent = parent(&journal)?;
        parent.assert_path(Path::new(&journal.plan.parent))?;
        let root = stage(&parent, &journal, &journal.stage_name)?;
        // npm ci is destructive: an install launch only accepts an absent tree
        // in this positively owned, unpublished workspace.
        if root.entry("node_modules")?.is_some() {
            return Err(io(
                "This workspace already contains dependencies; use a new reviewed retry plan",
            ));
        }
        filesystem::verify_tree(&root, &journal.files, &journal.directories)?;
        let (package, _) = root.read("package.json", 1024 * 1024)?;
        let (lock, lock_receipt) = root.read("package-lock.json", 1024 * 1024)?;
        let package: serde_json::Value = serde_json::from_slice(&package).map_err(io)?;
        let lock: serde_json::Value = serde_json::from_slice(&lock).map_err(io)?;
        super::super::locks::validate(&package, &lock)?;
        if lock_receipt.sha256 != status.lockfile_sha256 {
            return Err(io("The lockfile changed since authorization"));
        }
        let prepared = node_projects::prepare_creation_tools(
            layout,
            status.runtime.clone(),
            Path::new(&journal.plan.parent)
                .join(&journal.stage_name)
                .as_path(),
            Path::new(&journal.plan.parent)
                .join(&journal.stage_name)
                .as_path(),
            &journal.plan.template.node_range,
            &journal.plan.template.package_manager.version_range,
        )?;
        if prepared.summary().runtime_revision != journal.plan.tools.revision
            || prepared.summary().npm.cli_path != journal.plan.tools.npm_cli_path
            || prepared.summary().npm.version != journal.plan.tools.npm_version
        {
            return Err(io("Selected Node/npm changed"));
        }
        let source_data = Dir::absolute(&std::fs::canonicalize(&layout.data).map_err(io)?)?;
        let work = match source_data.entry("node-installations")? {
            Some(_) => source_data.child("node-installations")?,
            None => source_data.make("node-installations")?,
        };
        work.private()?;
        let jobs = match work.entry("workspaces")? {
            Some(_) => work.child("workspaces")?,
            None => work.make("workspaces")?,
        };
        jobs.private()?;
        let tools = jobs.make(job_id)?;
        let _cache = tools.make("cache")?;
        let _home = tools.make("home")?;
        let _tmp = tools.make("tmp")?;
        let base = std::fs::canonicalize(&layout.data)
            .map_err(io)?
            .join("node-installations/workspaces")
            .join(job_id);
        let mut command = prepared.node_command();
        command
            .arg(&journal.plan.tools.npm_cli_path)
            .args([
                "ci",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                "--include=dev",
                "--global=false",
                "--workspaces=false",
                "--strict-ssl=true",
                "--engine-strict=true",
                "--fetch-retries=1",
                "--fetch-timeout=15000",
                "--maxsockets=4",
            ])
            .arg(format!("--registry={}", status.registry));
        command
            .env("HOME", base.join("home"))
            .env("TMPDIR", base.join("tmp"))
            .env("npm_config_cache", base.join("cache"));
        // Pin cwd to the already validated inode. A pathname replacement after
        // validation must never redirect npm ci into a different project.
        let pinned = root.file()?;
        unsafe {
            command.pre_exec(move || {
                if libc::fchdir(pinned.as_raw_fd()) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        Ok(command)
    })()
    .map_err(node_error)
}
fn update(
    service: &NodeProjectCreation,
    store: &Store,
    journal: &mut Journal,
    state: InstallState,
    error: Option<Error>,
) -> Result<InstallStatus> {
    let status = journal
        .installation
        .as_mut()
        .ok_or_else(|| io("Missing installation status"))?;
    if let Some(slot) = service
        .installation
        .slots
        .lock()
        .map_err(|_| unavailable())?
        .get(&status.job_id)
    {
        status.cancel_requested = slot.cancel.load(Ordering::SeqCst);
    }
    status.state = state;
    status.error = error;
    status.revision += 1;
    let result = status.clone();
    store.save(journal)?;
    service.installation.update(&result, true);
    Ok(result)
}
// A synchronous rejection can happen after the durable journal was claimed.
// Keep that discoverable job terminal even though no worker was launched.
fn reject_start(service: &NodeProjectCreation, status: &InstallStatus, error: Error) -> Error {
    let state = if error.code == ErrorCode::GenerationCancelled {
        InstallState::Cancelled
    } else {
        InstallState::Failed
    };
    let mut failed = status.clone();
    failed.state = state.clone();
    failed.error = Some(error.clone());
    failed.revision += 1;
    let persisted = (|| -> Result<InstallStatus> {
        let (store, journal) = find(service, &status.job_id)?;
        let _lock = store.lock()?;
        let mut journal = store.load(&journal.plan.id)?;
        update(service, &store, &mut journal, state, Some(error.clone()))
    })();
    if let Ok(status) = persisted {
        failed = status;
    }
    service.installation.update(&failed, false);
    error
}
fn publish(
    service: &NodeProjectCreation,
    nodes: &node_projects::NodeProjects,
    store: &Store,
    journal: &mut Journal,
) -> Result<()> {
    let parent = parent(journal)?;
    parent.assert_path(Path::new(&journal.plan.parent))?;
    if parent.entry(&journal.stage_name)?.is_some() {
        let root = stage(&parent, journal, &journal.stage_name)?;
        verify_project(&root, journal)?;
        if parent.entry(&journal.plan.project_name)?.is_some() {
            return Err(Error::new(
                ErrorCode::DestinationExists,
                "Publication target became occupied; installed workspace was retained.",
            ));
        }
        journal.phase = Phase::Publishing;
        store.save(journal)?;
        parent.verify_child(&journal.stage_name, &root)?;
        parent.no_replace(&journal.stage_name, &journal.plan.project_name)?;
    }
    journal.phase = Phase::NeedsRegistration;
    store.save(journal)?;
    register(service, nodes, store, journal)
}
impl NodeProjectCreation {
    pub fn start_installation(
        self: &Arc<Self>,
        nodes: Arc<node_projects::NodeProjects>,
        request: InstallRequest,
    ) -> Result<InstallStatus> {
        self.layout()?;
        if !request.confirmed || !request.install_confirmed {
            return Err(Error::new(ErrorCode::ConfirmationRequired,"Installing pinned dependencies requires explicit authorization separate from generation."));
        }
        let plan = self.validate_plan(super::super::PlanReference {
            plan_id: request.plan_id.clone(),
            digest: request.digest.clone(),
        })?;
        if !self.installation.accepting.load(Ordering::SeqCst) {
            return Err(io("Installation is shutting down"));
        }
        let job_id = format!("{:032x}", rand::random::<u128>());
        let run_id = format!("{:032x}", rand::random::<u128>());
        #[allow(unused_mut)]
        let mut registry = "https://registry.npmjs.org/".to_string();
        #[cfg(all(debug_assertions, feature = "node-install-fixtures"))]
        if let Some(value) = self
            .installation
            .fixture_registry
            .lock()
            .map_err(|_| unavailable())?
            .clone()
        {
            registry = value;
        }
        if !registry_allowed(&registry) {
            return Err(io("Only the approved registry is supported"));
        }
        let status = InstallStatus {
            job_id: job_id.clone(),
            creation_id: plan.id.clone(),
            run_id: run_id.clone(),
            revision: 1,
            state: InstallState::Starting,
            runtime: node_projects::ProjectRuntime {
                node_path: plan.tools.node_path.clone(),
                version: plan.tools.node_version.clone(),
            },
            lockfile_sha256: plan
                .files
                .get("package-lock.json")
                .ok_or_else(|| io("Missing reviewed lockfile"))?
                .sha256
                .clone(),
            registry,
            ci_completed: false,
            cancel_requested: false,
            exit_code: None,
            error: None,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut slots = self.installation.slots.lock().map_err(|_| unavailable())?;
            if slots.values().any(|slot| slot.active) {
                return Err(Error::new(
                    ErrorCode::InstallBusy,
                    "One dependency installation may run at a time.",
                ));
            }
            slots.clear();
            slots.insert(
                job_id.clone(),
                Slot {
                    status: status.clone(),
                    cancel: cancel.clone(),
                    active: true,
                    publishing: false,
                },
            );
        }
        let generated = self.stage_install(
            &nodes,
            GenerateRequest {
                plan_id: request.plan_id,
                digest: request.digest,
                confirmed: true,
            },
            status.clone(),
        );
        match generated {
            Ok(job) if job.phase == Phase::Ready => (),
            Ok(job) => {
                return Err(reject_start(
                    self,
                    &status,
                    job.error
                        .unwrap_or_else(|| io("Could not prepare private staging")),
                ));
            }
            Err(error) => {
                return Err(reject_start(self, &status, error));
            }
        }
        let service = self.clone();
        let worker_status = status.clone();
        if let Err(error) = thread::Builder::new()
            .name("fairway-npm-install".into())
            .spawn(move || worker(service, nodes, worker_status, cancel))
        {
            return Err(reject_start(self, &status, io(error)));
        }
        Ok(status)
    }
    pub fn installation_status(&self, request: InstallQuery) -> Result<InstallStatus> {
        self.layout()?;
        if let Some(slot) = self
            .installation
            .slots
            .lock()
            .map_err(|_| unavailable())?
            .get(&request.job_id)
        {
            return Ok(slot.status.clone());
        }
        let (_, journal) = find(self, &request.job_id)?;
        let mut status = journal.installation.ok_or_else(|| io("Missing status"))?;
        if matches!(
            status.state,
            InstallState::Starting | InstallState::Installing
        ) {
            status.state = InstallState::Interrupted;
            status.error=Some(io("The previous host session ended. No process was adopted or automatically restarted."));
        }
        Ok(status)
    }
    pub fn installation_logs(&self, request: InstallLogRequest) -> Result<node_projects::LogBatch> {
        let status = self.installation_status(InstallQuery {
            job_id: request.job_id.clone(),
        })?;
        self.installation
            .executions
            .logs(node_projects::LogRequest {
                id: request.job_id,
                run_id: status.run_id,
                after_cursor: request.after_cursor,
            })
            .map_err(Into::into)
    }
    pub fn cancel_installation(&self, request: InstallReference) -> Result<InstallStatus> {
        self.layout()?;
        let (creation_id, run_id) = {
            let mut slots = self.installation.slots.lock().map_err(|_| unavailable())?;
            let slot = slots
                .get_mut(&request.job_id)
                .filter(|slot| slot.active)
                .ok_or_else(|| {
                    Error::new(ErrorCode::JobUnavailable, "No matching active installer.")
                })?;
            if slot.status.revision != request.expected_revision {
                return Err(Error::new(
                    ErrorCode::StaleInstallation,
                    "Installation changed; refresh before cancellation.",
                ));
            }
            if slot.publishing {
                return Err(Error::new(
                    ErrorCode::CancellationTooLate,
                    "Publication already began; the project is retained.",
                ));
            }
            slot.cancel.store(true, Ordering::SeqCst);
            slot.status.cancel_requested = true;
            (slot.status.creation_id.clone(), slot.status.run_id.clone())
        };
        if let Some(active) = self.active.lock().map_err(|_| unavailable())?.as_ref() {
            if active.id == creation_id {
                active.cancelled.store(true, Ordering::SeqCst);
            }
        }
        // The run may still be starting; the worker observes cancellation before launch too.
        if let Err(error) = self
            .installation
            .executions
            .stop(node_projects::StopRequest {
                id: request.job_id.clone(),
                run_id,
            })
        {
            if error.code != node_projects::ErrorCode::RunNotFound {
                return Err(error.into());
            }
        }
        self.installation_status(InstallQuery {
            job_id: request.job_id,
        })
    }
    pub fn recover_installation(
        &self,
        nodes: &node_projects::NodeProjects,
        request: InstallRecoveryRequest,
    ) -> Result<InstallStatus> {
        self.layout()?;
        if !request.confirmed {
            return Err(Error::new(
                ErrorCode::ConfirmationRequired,
                "Recovery requires explicit confirmation.",
            ));
        }
        let (store, mut journal) = find(self, &request.job_id)?;
        self.installation.ensure_creation_idle(&journal.plan.id)?;
        let _operation = self
            .operations
            .try_lock()
            .map_err(|_| Error::new(ErrorCode::InstallBusy, "Creation/recovery is busy."))?;
        let _journal_lock = store.lock()?;
        journal = store.load(&journal.plan.id)?;
        let status = journal.installation.as_ref().unwrap();
        if status.revision != request.expected_revision
            || matches!(status.state, InstallState::Completed)
        {
            return Err(Error::new(
                ErrorCode::StaleInstallation,
                "Installation changed or is already completed.",
            ));
        }
        let parent = parent(&journal)?;
        let published = parent
            .entry(&journal.plan.project_name)?
            .is_some_and(|(id, _)| Some(id) == journal.stage_identity);
        let root = stage(
            &parent,
            &journal,
            if published {
                &journal.plan.project_name
            } else {
                &journal.stage_name
            },
        )?;
        let _runner_lock = root.lock_existing(".fairway-install.lock")?;
        let result = match request.action {
            InstallRecoveryAction::Finalize => {
                if !status.ci_completed {
                    return Err(io("There is no verified completed installation. Review a new retry plan; partial data is preserved."));
                }
                verify_project(&root, &journal)?;
                journal.expected_registry_revision = nodes.snapshot().revision;
                publish(self, nodes, &store, &mut journal)?;
                update(self, &store, &mut journal, InstallState::Completed, None)
            }
            InstallRecoveryAction::DiscardStaging => {
                if published {
                    return Err(Error::new(
                        ErrorCode::CancellationTooLate,
                        "A published project is never deleted by installation recovery.",
                    ));
                }
                if root.entry("node_modules")?.is_some() {
                    return Err(Error::new(ErrorCode::RecoveryUnsafe,"Dependency data may be user-modified or partial. It was preserved; retry uses a new reviewed workspace."));
                }
                discard(&store, &mut journal)?;
                update(self, &store, &mut journal, InstallState::Cancelled, None)
            }
        };
        if let Ok(status) = &result {
            self.installation.update(status, false);
        }
        result
    }
}
fn worker(
    service: Arc<NodeProjectCreation>,
    nodes: Arc<node_projects::NodeProjects>,
    mut status: InstallStatus,
    cancel: Arc<AtomicBool>,
) {
    let result = (|| -> Result<()> {
        let _operation = service.operations.lock().map_err(|_| unavailable())?;
        let (store, mut journal) = find(&service, &status.job_id)?;
        let _store_lock = store.lock()?;
        if cancel.load(Ordering::SeqCst) {
            update(
                &service,
                &store,
                &mut journal,
                InstallState::Cancelled,
                None,
            )?;
            return Ok(());
        }
        let layout = service.layout()?;
        service.installation.executions.start(
            node_projects::Launch {
                home: layout.home.clone(),
                data: layout.data.join("node-installations"),
                runtime: layout.runtime.clone(),
                id: status.job_id.clone(),
                run_id: status.run_id.clone(),
                expected_revision: String::new(),
                manifest_revision: String::new(),
                installation: Some(node_projects::InstallLaunch {
                    creation_data: layout.data.clone(),
                    creation_id: status.creation_id.clone(),
                }),
            },
            status.runtime.clone(),
        )?;
        let mut observed_running = false;
        let run = loop {
            let run = service
                .installation
                .executions
                .statuses()
                .into_iter()
                .find(|run| run.project_id == status.job_id)
                .ok_or_else(|| io("Lost owned install status"))?;
            if run.state == node_projects::RunState::Running && !observed_running {
                status = update(
                    &service,
                    &store,
                    &mut journal,
                    InstallState::Installing,
                    None,
                )?;
                observed_running = true;
            }
            if cancel.load(Ordering::SeqCst)
                && matches!(
                    run.state,
                    node_projects::RunState::Starting | node_projects::RunState::Running
                )
            {
                let _ = service
                    .installation
                    .executions
                    .stop(node_projects::StopRequest {
                        id: status.job_id.clone(),
                        run_id: status.run_id.clone(),
                    });
            }
            if matches!(
                run.state,
                node_projects::RunState::Exited
                    | node_projects::RunState::Failed
                    | node_projects::RunState::Stopped
            ) {
                break run;
            }
            thread::sleep(Duration::from_millis(25));
        };
        let run = service
            .installation
            .executions
            .wait_for_exit(node_projects::StopRequest {
                id: status.job_id.clone(),
                run_id: run.run_id,
            })?;
        status.exit_code = run.exit_code;
        journal.installation.as_mut().unwrap().exit_code = run.exit_code;
        if cancel.load(Ordering::SeqCst) {
            status = update(
                &service,
                &store,
                &mut journal,
                InstallState::Cancelled,
                None,
            )?;
            return Ok(());
        }
        if run.state != node_projects::RunState::Exited
            || run.exit_code != Some(0)
            || run.forced_cleanup
        {
            return Err(io(run
                .error
                .map(|error| error.message)
                .unwrap_or_else(|| format!("npm ci exited {:?}", run.exit_code))));
        }
        let parent = parent(&journal)?;
        parent.assert_path(Path::new(&journal.plan.parent))?;
        let root = stage(&parent, &journal, &journal.stage_name)?;
        let _workspace_lock = root.lock_existing(".fairway-install.lock")?;
        let dependencies = dependency_snapshot(&root)?;
        filesystem::verify_tree_with_dependencies(
            &root,
            &journal.files,
            &journal.directories,
            dependencies
                .as_ref()
                .map(|dep| Identity {
                    dev: dep.dev,
                    ino: dep.ino,
                })
                .as_ref(),
        )?;
        journal.dependencies = dependencies;
        journal.installation.as_mut().unwrap().ci_completed = true;
        store.save(&mut journal)?;
        if cancel.load(Ordering::SeqCst) {
            status = update(
                &service,
                &store,
                &mut journal,
                InstallState::Cancelled,
                None,
            )?;
            return Ok(());
        }
        {
            let mut slots = service
                .installation
                .slots
                .lock()
                .map_err(|_| unavailable())?;
            let slot = slots.get_mut(&status.job_id).ok_or_else(unavailable)?;
            if slot.cancel.load(Ordering::SeqCst) {
                drop(slots);
                status = update(
                    &service,
                    &store,
                    &mut journal,
                    InstallState::Cancelled,
                    None,
                )?;
                return Ok(());
            }
            slot.publishing = true;
        }
        publish(&service, &nodes, &store, &mut journal)?;
        status = update(
            &service,
            &store,
            &mut journal,
            InstallState::Completed,
            None,
        )?;
        Ok(())
    })();
    // Always drain our owned run before releasing the runtime/workspace lease,
    // including errors such as a journal-save failure while npm is running.
    if let Err(cleanup) = service
        .installation
        .executions
        .stop(node_projects::StopRequest {
            id: status.job_id.clone(),
            run_id: status.run_id.clone(),
        })
    {
        if cleanup.code != node_projects::ErrorCode::RunNotFound {
            status.state = InstallState::Failed;
            status.error = Some(io(cleanup.message));
            service.installation.update(&status, true);
            return;
        }
    }
    if let Err(error) = result {
        status.state = InstallState::Failed;
        status.error = Some(error.clone());
        status.revision += 1;
        let persisted = (|| -> Result<InstallStatus> {
            let (store, journal) = find(&service, &status.job_id)?;
            let _lock = store.lock()?;
            let mut journal = store.load(&journal.plan.id)?;
            // Preserve the observed exit even when verification/publication
            // fails before its journal update could be committed.
            journal.installation.as_mut().unwrap().exit_code = status.exit_code;
            update(
                &service,
                &store,
                &mut journal,
                InstallState::Failed,
                Some(error),
            )
        })();
        if let Ok(updated) = persisted {
            status = updated;
        }
    } else if let Ok((_, journal)) = find(&service, &status.job_id) {
        if let Some(updated) = journal.installation {
            status = updated;
        }
    }
    service.installation.update(&status, false);
}

// After a host crash the old helper can still be doing bounded cleanup. Its
// workspace flock, not a saved PID, remains authority for runtime protection.
pub(super) fn ensure_recovered_runtime_unused(
    service: &NodeProjectCreation,
    node: &Path,
) -> Result<()> {
    let node = std::fs::canonicalize(node).unwrap_or_else(|_| node.into());
    let Some(store) = Store::open(service.layout()?, false)? else {
        return Ok(());
    };
    let (journals, errors) = store.list()?;
    if !errors.is_empty() {
        return Err(io("Creation journals need review before runtime removal"));
    }
    for journal in journals {
        let Some(status) = &journal.installation else {
            continue;
        };
        if Path::new(&status.runtime.node_path) != node
            || !matches!(
                status.state,
                InstallState::Starting | InstallState::Installing
            )
        {
            continue;
        }
        if journal.stage_identity.is_none() {
            continue;
        }
        let parent = parent(&journal)?;
        let published = parent
            .entry(&journal.plan.project_name)?
            .is_some_and(|(id, _)| Some(id) == journal.stage_identity);
        let root = stage(
            &parent,
            &journal,
            if published {
                &journal.plan.project_name
            } else {
                &journal.stage_name
            },
        )?;
        let _lock=root.lock_existing(".fairway-install.lock").map_err(|_|Error::new(ErrorCode::InstallBusy,"An installer is still cleaning up, or its ownership cannot be verified. Runtime removal was blocked."))?;
    }
    Ok(())
}
