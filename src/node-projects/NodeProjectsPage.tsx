import { useRef, useState } from 'react';
import { Code2, ExternalLink, FolderOpen, Pencil, Play, Plus, RefreshCw, ScrollText, Square, Trash2 } from 'lucide-react';
import Dialog from '../data/Dialog';
import { canControl, invoke } from '../bridge';
import * as api from './api';
import { activeRun, localUrl, nodeError } from './model';
import { useProjects } from './useProjects';
import ProjectEditor from './ProjectEditor';
import ProjectLogs from './ProjectLogs';
import type { NodeProjectConfig, NodeProjectInspection, NodeRunStatus, SaveNodeProjectRequest } from './types';
import './projects.css';

type Confirmation = { kind: 'start' | 'restart'; project: NodeProjectConfig; run?: NodeRunStatus; inspection: NodeProjectInspection; revision: string } | { kind: 'remove'; project: NodeProjectConfig; revision: string };
const states = { starting: 'Starting', running: 'Running', stopping: 'Stopping', stopped: 'Stopped', exited: 'Exited', failed: 'Failed' };
export default function NodeProjectsPage({ active, busy }: { active: boolean; busy: boolean }) {
  const { snapshot, runs, error, loading, refresh } = useProjects(active);
  const [editor, setEditor] = useState<{ project: NodeProjectConfig | null; revision: string } | null>(null);
  const [confirm, setConfirm] = useState<Confirmation | null>(null);
  const [dialogError, setDialogError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const operation = useRef(false);
  const [notice, setNotice] = useState<{ error: boolean; text: string } | null>(null);
  const [logs, setLogs] = useState<string | null>(null);
  const importButton = useRef<HTMLButtonElement>(null);
  const modalTrigger = useRef<HTMLElement | null>(null);
  function trigger(element: HTMLElement) { element.focus({ preventScroll: true }); modalTrigger.current = element; }
  const blocked = pending || busy || loading;
  async function perform(action: () => Promise<void>, rethrow = false) {
    if (operation.current || busy) return;
    operation.current = true; setPending(true); setNotice(null);
    try { await action(); }
    catch (error) { if (!editor && !confirm) setNotice({ error: true, text: nodeError(error) }); if (rethrow) throw error; }
    finally { operation.current = false; setPending(false); await refresh(); }
  }
  async function execution(project: NodeProjectConfig, kind: 'start' | 'restart') {
    await perform(async () => {
      const inspection = await api.inspectNodeProject({ root: project.root });
      if (!inspection.packageManager.npmSupported) throw new Error(`This project uses ${inspection.packageManager.name}. Execution currently supports npm only. Importing did not run anything.`);
      if (!(project.script in inspection.scripts)) throw new Error('The selected script is missing. Edit the project and choose a current script.');
      const current = await api.nodeProjectsSnapshot();
      if (current.registryError) throw current.registryError;
      const saved = current.projects.find(item => item.config.id === project.id)?.config;
      if (!saved || JSON.stringify(saved) !== JSON.stringify(project)) throw new Error('This registration changed. Refresh projects before starting it.');
      setDialogError(null); setConfirm({ kind, project, run: runs[project.id], inspection, revision: current.revision! });
    });
  }
  async function save(request: SaveNodeProjectRequest, npmSupported: boolean) {
    await perform(async () => {
      const saved = await api.saveNodeProject(request);
      setEditor(null);
      let text = 'Project saved. It has not been started.';
      let warning = false;
      if (npmSupported) {
        try { await api.prepareNodeRuntime({ id: saved.project.id, expectedRevision: saved.revision, manifestRevision: request.manifestRevision }); text += ' Node and npm selection verified.'; }
        catch (error) { warning = true; text += ` Runtime selection needs attention: ${nodeError(error)} Use Edit to choose a compatible runtime.`; }
      } else text += ' This package manager is unsupported for execution; npm projects are supported.';
      setNotice({ error: warning, text });
    }, true);
  }
  async function confirmed() {
    if (!confirm) return;
    const choice = confirm;
    await perform(async () => {
      if (choice.kind === 'remove') {
        await api.removeNodeProject({ id: choice.project.id, expectedRevision: choice.revision });
        if (logs === choice.project.id) setLogs(null);
        setNotice({ error: false, text: 'Registration removed. Your project files were kept.' });
        queueMicrotask(() => importButton.current?.focus());
      } else {
        const request = { id: choice.project.id, expectedRevision: choice.revision, manifestRevision: choice.inspection.manifestRevision, confirmed: true as const };
        if (choice.kind === 'restart') {
          if (!choice.run) throw new Error('The run is unavailable. Refresh its status.');
          await api.restartNodeProject({ ...request, runId: choice.run.runId });
        } else await api.startNodeProject(request);
        setLogs(choice.project.id);
      }
      setConfirm(null); setDialogError(null);
    }, true).catch(error => setDialogError(nodeError(error)));
  }
  const selected = snapshot?.projects.find(item => item.config.id === logs)?.config;
  const selectedRun = logs ? runs[logs] : undefined;
  // Native metadata is retained after removal. Show orphaned active runs for
  // cleanup, and retained history when the registry itself cannot be loaded.
  const unlisted = Object.values(runs).filter(run => !snapshot?.projects.some(item => item.config.id === run.projectId)
    && (activeRun(run) || !!snapshot?.registryError));
  const supported = canControl && snapshot?.supported;
  // The page owns no processes. Navigation and component teardown never send Stop.
  return <div className="node-projects-page">
    <div className="page-heading"><div><h1>Node Projects</h1><p>Your existing Node.js apps, each with its own runtime.</p></div><div className="button-row"><button className="secondary" disabled={blocked || !canControl || loading} onClick={() => { void refresh(); }}><RefreshCw size={14} />Refresh</button><button ref={importButton} className="primary" disabled={blocked || !supported || !!snapshot?.registryError || !snapshot?.revision} onClick={event => { trigger(event.currentTarget); setEditor({ project: null, revision: snapshot!.revision! }); }}><Plus size={15} />Import Node project</button></div></div>
    <section className="sites-server"><Code2 size={20} /><div><strong>Explicit starts. Independent runtimes.</strong><p>Importing never runs scripts. Start and Restart execute the selected npm script and its lifecycle hooks. PHP service controls stay separate. Running indicates a process, not application readiness.</p></div></section>
    {!canControl && <div className="notice" role="status">Open the macOS desktop app to manage Node projects.</div>}
    {canControl && snapshot && !snapshot.supported && <div className="notice" role="status">Node Projects is available on macOS first. Your PHP services remain available.</div>}
    {loading && !snapshot && <p role="status">Loading Node projects…</p>}
    {(error || snapshot?.registryError) && <div className="notice error" role="alert">{error ?? nodeError(snapshot!.registryError)} <button className="text-link" disabled={blocked} onClick={() => { void refresh(); }}>Retry loading projects</button></div>}
    {notice && <div className={`notice${notice.error ? ' error' : ''}`} role={notice.error ? 'alert' : 'status'}>{notice.text}</div>}
    {supported && !snapshot.registryError && !snapshot.projects.length && <section className="sites-empty"><FolderOpen size={26} /><h2>No Node projects yet</h2><p>Import a folder containing package.json. Choose an installed Node version and an npm script, then start it when you are ready.</p></section>}
    {supported && <ul className="sites-list node-project-list" aria-label="Registered Node projects">{snapshot.projects.map(({ config: project, rootAvailable, manifestAvailable, runtimeAvailable }) => {
      const run = runs[project.id], running = activeRun(run);
      return <li className="site-item node-project-card" key={project.id} aria-label={project.name}>
        <div className="site-title"><h2>{project.name}</h2><span role="status" className={`site-status${run?.state === 'running' ? ' ready' : ''}`}>{run ? states[run.state] : 'Stopped'}</span></div>
        <p className="site-root"><span>Folder</span><code>{project.root}</code></p>
        <div className="node-project-meta"><span>Node <strong>{project.runtime.version}</strong></span><span>npm script <code>{project.script}</code></span></div>
        {project.openUrl && <p className="node-project-url"><code>{project.openUrl}</code><button className="text-link" disabled={blocked || !localUrl(project.openUrl)} aria-label={`Open URL for ${project.name}`} onClick={() => { void perform(async () => { await invoke('open_mail_link', { url: project.openUrl }); }); }}><ExternalLink size={13} />Open URL</button></p>}
        {(!rootAvailable || !manifestAvailable || !runtimeAvailable) && <p className="site-warning">{!rootAvailable ? 'Project folder is missing. Edit to choose its new location.' : !manifestAvailable ? 'package.json is missing. Restore it or choose another folder in Edit.' : 'Selected runtime is missing. Edit and choose an installed version.'}</p>}
        {run?.error && <p className="site-warning" role="alert">{nodeError(run.error)}</p>}
        {run && ['exited', 'failed'].includes(run.state) && run.exitCode !== null && <p className="site-warning">Process exited with code {run.exitCode}.</p>}
        <div className="node-project-actions">
          <button className="primary" disabled={blocked || running || !rootAvailable || !manifestAvailable || !runtimeAvailable || !!snapshot.registryError} aria-label={`Start ${project.name}`} onClick={event => { trigger(event.currentTarget); void execution(project, 'start'); }}><Play size={13} />Start</button>
          <button className="secondary" disabled={blocked || !running} aria-label={`Stop ${project.name}`} onClick={() => { if (run) void perform(async () => { await api.stopNodeProject({ id: project.id, runId: run.runId }); }); }}><Square size={13} />Stop</button>
          <button className="secondary" disabled={blocked || !run || ['starting', 'stopping'].includes(run.state) || !rootAvailable || !manifestAvailable || !runtimeAvailable || !!snapshot.registryError} aria-label={`Restart ${project.name}`} onClick={event => { trigger(event.currentTarget); void execution(project, 'restart'); }}><RefreshCw size={13} />Restart</button>
          <button className="secondary" disabled={!run} aria-label={`Logs for ${project.name}`} onClick={() => setLogs(project.id)}><ScrollText size={13} />Logs</button>
          <button className="secondary" disabled={blocked || running || !!snapshot.registryError} aria-label={`Edit ${project.name}`} onClick={event => { trigger(event.currentTarget); setEditor({ project, revision: snapshot.revision! }); }}><Pencil size={13} />Edit</button>
          <button className="secondary node-remove" disabled={blocked || running || !!snapshot.registryError} aria-label={`Remove ${project.name}`} onClick={event => { trigger(event.currentTarget); setDialogError(null); setConfirm({ kind: 'remove', project, revision: snapshot.revision! }); }}><Trash2 size={13} />Remove</button>
        </div>
      </li>;
    })}</ul>}
    {supported && unlisted.length > 0 && <section className="node-unlisted" aria-label="Unlisted project runs"><h2>Runs without a loaded registration</h2><p>You can still stop a managed run or read its logs while the registry needs attention.</p>{unlisted.map(run => <div className="site-item" key={run.projectId}><h3>Project {run.projectId.slice(0, 8)}</h3><p role="status">{states[run.state]} · Node {run.runtime.version}</p><div className="button-row"><button className="secondary" disabled={blocked || !activeRun(run)} aria-label={`Stop unlisted ${run.projectId}`} onClick={() => { void perform(async () => { await api.stopNodeProject({ id: run.projectId, runId: run.runId }); }); }}>Stop</button><button className="secondary" onClick={() => setLogs(run.projectId)}>Logs</button></div></div>)}</section>}
    {selectedRun && <ProjectLogs key={`${selectedRun.projectId}:${selectedRun.runId}`} name={selected?.name ?? `Project ${selectedRun.projectId.slice(0, 8)}`} run={selectedRun} active={active} />}
    {editor && <ProjectEditor returnFocus={modalTrigger.current} project={editor.project} revision={editor.revision} busy={blocked} onSave={save} onClose={() => setEditor(null)} />}
    {confirm && <Dialog returnFocus={modalTrigger.current} title={confirm.kind === 'remove' ? 'Remove Node project?' : confirm.kind === 'restart' ? 'Restart Node project?' : 'Start Node project?'} busy={blocked} onClose={() => setConfirm(null)} onSubmit={() => { void confirmed(); }} submit={confirm.kind === 'remove' ? 'Remove registration' : confirm.kind === 'restart' ? 'Restart project' : 'Start project'} danger={confirm.kind === 'remove'}>
      <p><strong>{confirm.project.name}</strong></p>
      {dialogError && <div className="notice error" role="alert">{dialogError}</div>}
      {confirm.kind === 'remove' ? <p>Remove this registration from Fairway? Your project folder and its files will be kept.</p> : <div className="node-project-form"><p>This will execute <code>npm run {confirm.project.script}</code> and its pre/post lifecycle hooks using Node {confirm.project.runtime.version}. Only start projects you trust.</p><pre className="node-script">{confirm.inspection.scripts[confirm.project.script]}</pre>{confirm.kind === 'restart' && <p>The current run will be stopped before a new run starts.</p>}<p>A running process does not confirm that the application is ready.</p></div>}
    </Dialog>}
  </div>;
}
