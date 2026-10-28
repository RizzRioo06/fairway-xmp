import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Eraser } from 'lucide-react';
import Workspace, { MaximizeWorkspace } from '../Workspace';
import { readNodeProjectLogs } from './api';
import { appendLogs, nodeError } from './model';
import type { NodeLogRecord, NodeRunStatus } from './types';

export default function ProjectLogs({ run, name, active }: { run: NodeRunStatus; name: string; active: boolean }) {
  const [view, setView] = useState({ records: [] as NodeLogRecord[], limited: false });
  const { records, limited } = view;
  const [follow, setFollow] = useState(true);
  const [gap, setGap] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [diskError, setDiskError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [visible, setVisible] = useState(!document.hidden);
  const [finished, setFinished] = useState(false);
  const cursor = useRef(0);
  const output = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const update = () => setVisible(!document.hidden);
    document.addEventListener('visibilitychange', update);
    return () => document.removeEventListener('visibilitychange', update);
  }, []);
  useEffect(() => {
    if (!active || !visible) return;
    let disposed = false;
    let timer = 0;
    async function poll() {
      try {
        const after = cursor.current;
        const batch = await readNodeProjectLogs({ id: run.projectId, runId: run.runId, afterCursor: after });
        if (disposed) return;
        setView(previous => {
          const merged = appendLogs(previous.records, batch.records, run.runId, after);
          return { records: merged.records, limited: previous.limited || merged.trimmed };
        });
        cursor.current = Math.max(after, batch.nextCursor);
        setGap(previous => previous || batch.truncated); setDiskError(batch.error);
        setError(null); setFinished(batch.eof);
        if (!batch.eof) timer = window.setTimeout(() => { void poll(); }, 500);
      } catch (error) { if (!disposed) setError(nodeError(error)); }
    }
    void poll();
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [run.projectId, run.runId, active, visible, retry]);
  useLayoutEffect(() => {
    if (follow && output.current) output.current.scrollTop = output.current.scrollHeight;
  }, [records, follow]);
  return <Workspace id="node-logs" title="Project logs" className="logs-workspace node-console">
    <div className="node-console-toolbar"><div><h2>Logs · {name}</h2><p>Run <code>{run.runId.slice(0, 8)}</code> · {finished ? 'Output complete' : 'Live output'}</p></div><div className="button-row">
      <label className="node-follow"><input type="checkbox" checked={follow} onChange={event => setFollow(event.target.checked)} />Follow output</label>
      <button className="secondary" onClick={() => setView({ records: [], limited: false })}><Eraser size={14} />Clear view</button><MaximizeWorkspace id="node-logs" title="Project logs" />
    </div></div>
    <div className="node-console-messages">
      {(gap || limited) && <p role="status">Earlier output is no longer in this view. Only recent output is retained.</p>}
      {diskError && <p role="alert">{diskError}</p>}
      {run.error && <p role="alert">{nodeError(run.error)}</p>}
      {(run.exitCode !== null || run.signal !== null) && <p>Process ended{run.exitCode !== null ? ` with exit code ${run.exitCode}` : ` with signal ${run.signal}`}.{run.forcedCleanup ? ' Unresponsive descendants required forced cleanup.' : ''}</p>}
      {error && <div role="alert">{error} <button className="text-link" onClick={() => setRetry(value => value + 1)}>Retry logs</button></div>}
    </div>
    <div ref={output} className="logs-output node-output" role="region" aria-label={`Output for ${name}`} tabIndex={0} onScroll={() => {
      const element = output.current;
      if (element) setFollow(element.scrollHeight - element.scrollTop - element.clientHeight < 32);
    }}>
      {!records.length && <p className="node-no-output">{error ? 'Log history is unavailable.' : 'No output in this view.'}</p>}
      {records.map(record => <div className={`log-entry node-log-${record.stream}`} key={record.cursor}><div className="log-entry-meta"><time dateTime={new Date(record.time).toISOString()}>{new Date(record.time).toLocaleTimeString()}</time><span>{record.stream}</span></div><pre>{record.text}</pre></div>)}
    </div>
  </Workspace>;
}
