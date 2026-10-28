import { useEffect, useId, useRef, useState } from 'react';
import { FolderOpen, RefreshCw } from 'lucide-react';
import Dialog from '../data/Dialog';
import { invoke } from '../bridge';
import { inspectNodeProject } from './api';
import { localUrl, nodeError } from './model';
import type { NodeProjectConfig, NodeProjectInspection, SaveNodeProjectRequest } from './types';

export default function ProjectEditor({ project, revision, busy, onSave, onClose, returnFocus }: {
  project: NodeProjectConfig | null; revision: string; busy: boolean; returnFocus: HTMLElement | null;
  onSave: (request: SaveNodeProjectRequest, npmSupported: boolean) => Promise<void>; onClose: () => void;
}) {
  const folderId = useId();
  const [name, setName] = useState(project?.name ?? '');
  const [root, setRoot] = useState(project?.root ?? '');
  const [script, setScript] = useState(project?.script ?? '');
  const [runtime, setRuntime] = useState('');
  const [url, setUrl] = useState(project?.openUrl ?? '');
  const [inspection, setInspection] = useState<NodeProjectInspection | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const sequence = useRef(0);
  const alive = useRef(true);
  async function inspect(path: string) {
    const request = ++sequence.current;
    setLoading(true); setError(null); setInspection(null); setRoot(path);
    try {
      const found = await inspectNodeProject({ root: path });
      if (!alive.current || request !== sequence.current) return;
      setRoot(found.root); setInspection(found);
      setName(previous => previous || found.name);
      setScript(previous => previous in found.scripts ? previous : 'dev' in found.scripts ? 'dev' : Object.keys(found.scripts)[0] ?? '');
      setRuntime(previous => found.runtimes.some(choice => choice.id === previous) ? previous : found.runtimes.find(choice => choice.nodePath === project?.runtime.nodePath && choice.version === project.runtime.version)?.id ?? found.suggestedRuntimeId ?? '');
    } catch (error) { if (alive.current && request === sequence.current) setError(nodeError(error)); }
    finally { if (alive.current && request === sequence.current) setLoading(false); }
  }
  useEffect(() => {
    alive.current = true;
    if (project) void inspect(project.root);
    return () => { alive.current = false; sequence.current++; };
  }, []);
  async function choose() {
    setLoading(true); setError(null);
    try {
      const folder = await invoke<string | null>('pick_folder');
      if (!alive.current) return;
      if (folder) await inspect(folder);
    } catch (error) { if (alive.current) setError(nodeError(error)); }
    finally { if (alive.current) setLoading(false); }
  }
  const valid = !!inspection && !!script && !!runtime && !!name.trim() && name.trim().length <= 80 && localUrl(url.trim());
  return <Dialog returnFocus={returnFocus} title={project ? 'Edit Node project' : 'Import Node project'} busy={busy || loading} onClose={onClose} submit="Save project" valid={valid}
    onSubmit={() => { if (inspection) void onSave({ config: { id: project?.id ?? null, name: name.trim(), root: inspection.root, script, runtimeId: runtime, openUrl: url.trim() || null }, expectedRevision: revision, manifestRevision: inspection.manifestRevision }, inspection.packageManager.npmSupported).catch(error => setError(nodeError(error))); }}>
    <div className="node-project-form">
      <p>Choose an existing folder containing <code>package.json</code>. Importing keeps your files in place and never runs scripts or installs dependencies.</p>
      <div className="site-field"><label htmlFor={folderId}>Project folder</label><span className="root-input"><input id={folderId} aria-label="Project folder" value={root} readOnly placeholder="Choose a project folder" /><button className="secondary" type="button" onClick={() => { void choose(); }}><FolderOpen size={14} />Choose folder…</button></span></div>
      {root && <button type="button" className="text-link" onClick={() => { void inspect(root); }}><RefreshCw size={13} />Refresh inspection and runtimes</button>}
      {loading && <p role="status">Inspecting project and installed runtimes…</p>}
      {error && <div className="notice error" role="alert">{error}</div>}
      {inspection && <>
        <label>Project name<input value={name} maxLength={80} required onChange={event => setName(event.target.value)} /></label>
        <label>npm script<select aria-label="npm script" value={script} required onChange={event => setScript(event.target.value)}>{!script && <option value="">Choose a script</option>}{Object.keys(inspection.scripts).map(name => <option key={name} value={name}>{name}</option>)}</select></label>
        {script && <pre className="node-script">{inspection.scripts[script]}</pre>}
        <div className="node-requirements"><strong>Node requirements</strong><dl>{[['package.json engines.node', inspection.versionHints.enginesNode], ['.node-version', inspection.versionHints.nodeVersion], ['.nvmrc', inspection.versionHints.nvmrc]].map(([label, value]) => <div key={label}><dt>{label}</dt><dd><code>{value || 'Not specified'}</code></dd></div>)}</dl></div>
        <label>Node runtime<select aria-label="Node runtime" required value={runtime} onChange={event => setRuntime(event.target.value)}><option value="">Choose an installed Node version</option>{inspection.runtimes.map(choice => <option value={choice.id} key={choice.id}>{choice.version} · {choice.source} · {choice.nodePath}</option>)}</select></label>
        <p>Node and npm compatibility is checked after saving and again before every start. Install additional versions in Settings → Node.js tools.</p>
        {!inspection.runtimes.length && <p role="alert">No installed Node runtimes found. Install a version in Settings, then refresh inspection.</p>}
        {project && !inspection.runtimes.some(choice => choice.nodePath === project.runtime.nodePath && choice.version === project.runtime.version) && <p role="alert">The saved runtime is unavailable or changed. Choose an installed version.</p>}
        <p>Package manager: <strong>{inspection.packageManager.declaration ?? inspection.packageManager.name}</strong>{inspection.packageManager.lockfiles.length ? ` · ${inspection.packageManager.lockfiles.join(', ')}` : ''}</p>
        {!inspection.packageManager.npmSupported && <div className="notice" role="status">This project uses {inspection.packageManager.name}. You can save its registration, but execution currently supports npm only. Fairway will not substitute another package manager.</div>}
        {inspection.runtimeError && <p role="alert">{inspection.runtimeError}</p>}
        {inspection.warnings.map(warning => <p className="site-warning" key={warning}>{warning}</p>)}
        <label>Local URL (optional)<input type="url" placeholder="http://localhost:3000/" value={url} onChange={event => setUrl(event.target.value)} aria-invalid={!!url && !localUrl(url.trim())} /></label>
        {!!url && !localUrl(url.trim()) && <p role="alert">Use HTTP or HTTPS with localhost, 127.0.0.1 or [::1], without credentials.</p>}
      </>}
    </div>
  </Dialog>;
}
