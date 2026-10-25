import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { CircleCheck, Download, LoaderCircle, RefreshCw, Trash2 } from 'lucide-react';
import { canControl, invoke } from './bridge';
import HelpTip from './HelpTip';
import type { NodeStatus } from './types';
import { useModalFocus } from './modal-focus';

type Props = {
  windows: boolean;
  disabled: boolean;
  onChange: (status: NodeStatus) => void;
  run: (label: string, action: () => Promise<void>) => Promise<void>;
};

function UninstallNodeDialog({ version, active, busy, onCancel, onConfirm }: {
  version: string; active: boolean; busy: boolean; onCancel: () => void; onConfirm: () => void;
}) {
  const root = useRef<HTMLDivElement>(null);
  const title = useId();
  const description = useId();
  useModalFocus(root, true, onCancel, { canClose: !busy });
  return <div className="modal-backdrop"><div ref={root} className="modal node-uninstall-dialog" role="dialog" aria-modal="true" aria-labelledby={title} aria-describedby={description} aria-busy={busy}>
    <div className="node-uninstall-heading"><span className="node-uninstall-icon" aria-hidden="true"><Trash2 size={19} /></span><h2 id={title}>Uninstall Node {version}?</h2></div>
    <p id={description}>This removes this Node version and its global npm packages.</p>
    {active && <div className="node-uninstall-notice">This is your active fnm version. Fairway will turn off its Node activation for new terminals.</div>}
    <p className="node-uninstall-hint">Stop terminals and development servers using this version before continuing.</p>
    <div className="node-uninstall-actions"><button className="secondary" disabled={busy} onClick={onCancel}>Cancel</button><button className="node-danger-button" disabled={busy} onClick={onConfirm}>{busy && <LoaderCircle className="spin" size={14} />}{busy ? 'Uninstalling…' : 'Uninstall Node'}</button></div>
  </div></div>;
}

export default function NodeTools({ windows, disabled, onChange, run }: Props) {
  const [status, setStatus] = useState<NodeStatus | null>(null);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(true);
  const [versions, setVersions] = useState<string[]>([]);
  const [selected, setSelected] = useState('');
  const [remoteLoading, setRemoteLoading] = useState(false);
  const [remove, setRemove] = useState<string | null>(null);
  const [managerOpen, setManagerOpen] = useState<boolean | null>(null);
  const live = useRef(true);
  const refreshRequest = useRef(0);
  const remoteRequest = useRef(false);
  const onChangeRef = useRef(onChange);
  const heading = useRef<HTMLHeadingElement>(null);
  const restoreAfterUninstall = useRef(false);
  onChangeRef.current = onChange;
  const external = status?.installations.filter(node => node.source !== 'fnm') ?? [];
  const usingManaged = !!(status?.enabled && status.fnmVersion && status.managed);
  const current = usingManaged ? status?.managed : status?.detected;
  const currentSource = usingManaged ? 'fnm' : status?.installations.find(node => windows ? node.path.toLowerCase() === current?.path.toLowerCase() : node.path === current?.path)?.source;
  const installAndUse = !!status && !current && !status.detectionError;

  const refresh = useCallback(async () => {
    const request = ++refreshRequest.current;
    setLoading(true); setError('');
    try {
      const next = await invoke<NodeStatus>('node_status');
      if (live.current && request === refreshRequest.current) { setStatus(next); onChangeRef.current(next); }
    } catch (cause) { if (live.current && request === refreshRequest.current) setError(String(cause)); }
    finally { if (live.current && request === refreshRequest.current) setLoading(false); }
  }, []);
  useEffect(() => { live.current = true; void refresh(); return () => { live.current = false; refreshRequest.current++; }; }, [refresh]);

  const loadVersions = async () => {
    if (remoteRequest.current) return;
    remoteRequest.current = true;
    setRemoteLoading(true); setError('');
    try {
      const next = await invoke<string[]>('node_versions');
      if (live.current) {
        setVersions(next);
        setSelected(current => next.includes(current) ? current : next[0] ?? '');
      }
    } catch (cause) { if (live.current) setError(String(cause)); }
    finally { remoteRequest.current = false; if (live.current) setRemoteLoading(false); }
  };
  const change = async (action: string, version = '') => {
    const label = action === 'install' ? `Installing Node ${version}…` : action === 'uninstall' ? `Uninstalling Node ${version}…` : 'Updating Node tools…';
    await run(label, async () => {
      try {
        await invoke('node_action', { action: action === 'use' ? 'default' : action, version });
        if (action === 'use' || (action === 'install' && installAndUse)) {
          if (action === 'install') await invoke('node_action', { action: 'default', version });
          await invoke('node_action', { action: 'enable', version: '' });
        }
      } finally { if (live.current) { setRemove(null); await refresh(); } }
    });
  };
  const unavailable = disabled || loading || !canControl;
  const removingActive = !!status?.enabled && remove === status.defaultVersion;
  useEffect(() => {
    if (restoreAfterUninstall.current && !remove && !disabled && !loading) {
      restoreAfterUninstall.current = false;
      heading.current?.focus({ preventScroll: true });
    }
  }, [remove, disabled, loading]);

  return <><section className="settings-card node-tools" aria-label="Node.js settings">
    <div className="node-heading">
      <div className="node-title"><h2 ref={heading} tabIndex={-1}>Node.js</h2><HelpTip label="Node.js help"><p>Fairway includes fnm. Node versions download when you install them.</p><p>Use selects a version for new Fairway terminals and, with CLI tools enabled, new {windows ? 'Command Prompt/PowerShell sessions' : 'Bash/Zsh shells'}. Existing terminals retain their version.</p>{windows && <p>PowerShell uses its normal profile script policy. Sessions started without profiles or with Command Prompt AutoRun disabled skip external shell activation.</p>}<p>For an older exact release, use <code>fnm install VERSION</code>. Project switching is not enabled automatically.</p></HelpTip></div>
      <button className="icon-button" disabled={unavailable} onClick={() => { void refresh(); }} aria-label="Refresh Node detection" title="Refresh Node detection"><RefreshCw size={15} className={loading ? 'spin' : undefined} /></button>
    </div>
    {current ? <div className="node-current">
      <CircleCheck className="node-current-icon" size={20} aria-hidden="true" />
      <div className="node-current-info"><span className="node-current-label">{usingManaged ? 'For new Fairway terminals' : windows ? 'Detected on your PATH' : 'Detected in your shell'}</span>
        <div className="node-current-version"><strong>Node {current.version}</strong>{currentSource && <span className="node-current-source">{currentSource === 'External' ? 'Existing installation' : currentSource}</span>}</div>
        <code className="node-current-path">{current.path}</code>
      </div>
    </div>
      : <p className="node-status" role="status">{loading ? 'Checking installed Node versions…' : !status ? 'Node status is unavailable.' : status.installations.length ? `${status.installations.length} installed versions found. ${windows ? 'No selection found on PATH.' : 'Your shell selection is unavailable.'}` : 'No Node found in the checked locations.'}</p>}
    {error && <p className="node-error" role="alert">{error}</p>}
    {status?.fnmError && <p className="node-error" role="alert">fnm: {status.fnmError}</p>}
    {status?.detectionError && <p className="node-status">{windows ? 'Node on PATH could not be checked.' : 'Shell selection could not be checked.'} Installed versions are listed below.</p>}
    {status && <details className="node-manager" open={managerOpen ?? installAndUse} onToggle={event => { setManagerOpen(event.currentTarget.open); if (event.currentTarget.open && status.fnmVersion && !versions.length) void loadVersions(); }}>
      <summary>Manage Node.js<span>{status.installed.length ? `${status.installed.length} installed with fnm` : 'Install with fnm'}</span></summary>
      {!status.fnmVersion ? <p className="node-status">Existing Node installations remain available. Reinstall the complete app to restore fnm management.</p> : <>
        <div className="node-install-row">
          <label htmlFor="node-release">Install a version</label>
          <div className="node-install-controls"><select id="node-release" value={selected} disabled={unavailable || remoteLoading} onChange={event => setSelected(event.target.value)}>
            {!versions.length && <option value="">{remoteLoading ? 'Loading releases…' : 'Releases unavailable'}</option>}
            {versions.map(version => <option key={version} value={version}>Node {version.split('.')[0].replace('v', '')} · {version}</option>)}
          </select>
          <button className="icon-button" disabled={unavailable || remoteLoading} onClick={() => { void loadVersions(); }} aria-label="Refresh available Node versions" title="Refresh releases"><RefreshCw size={14} className={remoteLoading ? 'spin' : undefined} /></button>
          <button className="secondary" disabled={unavailable || !selected || status.installed.includes(selected)} onClick={() => { void change('install', selected); }}>{remoteLoading ? <LoaderCircle className="spin" size={14} /> : <Download size={14} />}{status.installed.includes(selected) ? 'Installed' : installAndUse ? 'Install & use' : 'Install'}</button></div>
          <p>Latest release per major, from Node 20 onward. Includes LTS and non-LTS versions. Other exact versions are available through fnm.</p>
        </div>
        <div className="node-section-title">Installed with fnm</div>
        {status.installed.length ? <ul className="node-versions" aria-label="Installed Node versions">{status.installed.map(version => <li key={version}>
          <div className="node-version-name"><code>{version}</code>{version === status.defaultVersion && <span className={`node-badge${status.enabled ? '' : ' inactive'}`}>{status.enabled ? 'In use' : 'fnm default'}</span>}</div>
          <div className="node-version-actions">{(version !== status.defaultVersion || !status.enabled) && <button className="primary node-use-button" disabled={unavailable} onClick={() => { void change('use', version); }} aria-label={`Use Node ${version} in new terminals`}>Use</button>}<button className="secondary node-uninstall" disabled={unavailable} onClick={() => setRemove(version)} aria-label={`Uninstall Node ${version}`}>Uninstall</button></div>
        </li>)}</ul> : <p className="node-status">No versions installed with fnm yet.</p>}
        {status.enabled && <div className="node-external-action"><button className="text-link" disabled={unavailable} onClick={() => { void change('disable'); }}>{external.length ? 'Use existing Node setup' : 'Turn off Node activation'}</button></div>}
        {!!external.length && <details className="node-other"><summary>Other Node installations <span>{external.length}</span></summary><ul>{external.map(node => <li key={node.path}><div><strong>{node.source}</strong><code>{node.version}</code></div><span className="node-file-path">{node.path}</span></li>)}</ul><p>Managed outside Fairway. Use their original manager to update or uninstall them.</p></details>}
        <details className="node-other"><summary>Command details</summary><dl className="path-list"><div><dt>Bundled fnm</dt><dd>{status.fnmVersion} · {status.fnmPath}</dd></div><div><dt>Node storage</dt><dd>{status.directory}</dd></div><div><dt>{windows ? 'Detected on PATH' : 'Selected by your shell'}</dt><dd>{status.detected ? `${status.detected.version} · ${status.detected.path}` : status.detectionError ?? 'Not available'}</dd></div><div><dt>fnm default</dt><dd>{status.managed?.path ?? 'No installed default'}</dd></div></dl></details>
      </>}
    </details>}
  </section>{remove && <UninstallNodeDialog version={remove} active={removingActive} busy={disabled} onCancel={() => setRemove(null)} onConfirm={() => { restoreAfterUninstall.current = true; void change('uninstall', remove); }} />}</>;
}
