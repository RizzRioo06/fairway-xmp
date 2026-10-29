import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { appendLogs } from '../node-projects/model';
import type { NodeLogRecord } from '../node-projects/types';
import { nodeProjectInstallationLogs } from './api';
import { creationError, phaseLabel } from './model';
import type { CreationJobStatus } from './types';
export default function SetupProgress({ job, onRefresh, onCancel, pending }: { job?: CreationJobStatus; onRefresh: () => void; onCancel: () => void; pending: boolean }) {
  const installation = job?.installation;
  const [view, setView] = useState({ records: [] as NodeLogRecord[], limited: false });
  const [error, setError] = useState<string | null>(null);
  const [retry, setRetry] = useState(0), [follow, setFollow] = useState(true);
  const cursor = useRef(0), output = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!installation) return;
    let disposed = false, timer = 0;
    async function poll() {
      const after = cursor.current;
      try {
        const batch = await nodeProjectInstallationLogs({ jobId: installation!.jobId, afterCursor: after });
        if (disposed) return;
        setView(previous => { const merged = appendLogs(previous.records, batch.records, installation!.runId, after); return { records: merged.records, limited: previous.limited || merged.trimmed || batch.truncated }; });
        cursor.current = Math.max(after, batch.nextCursor); setError(batch.error);
        if (!batch.eof) timer = window.setTimeout(() => { void poll(); }, 500);
      } catch (error) {
        if (disposed) return;
        if (job?.active) timer = window.setTimeout(() => { void poll(); }, 1000);
        else setError(`No further installation output is available. ${creationError(error)}`);
      }
    }
    void poll();
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [installation?.jobId, installation?.runId, job?.active, retry]);
  useLayoutEffect(() => { if (follow && output.current) output.current.scrollTop = output.current.scrollHeight; }, [view.records, follow]);
  return <section className="creation-progress" aria-label="Project setup progress">
    <h4 role="status">{job ? phaseLabel(job) : 'Waiting for the native setup record'}</h4>
    {!job && <p>The request may still be preparing. Refresh status to reconnect; this view will not send a second creation request.</p>}
    {job?.active && <p>Fairway manages this setup. You can hide this dialog or navigate away; the job continues. No development server will be started.</p>}
    {job?.installation?.progress === 'verifying' && <p>npm has finished. Fairway is checking the generated files and installed dependencies.</p>}
    {job?.phase === 'needsRegistration' && <p>The project folder has been published. Fairway is saving its registration.</p>}
    <div className="button-row"><button type="button" className="secondary" onClick={onRefresh}>Refresh setup status</button>{job?.active && installation && <button type="button" className="secondary" disabled={pending || installation.cancelRequested} onClick={onCancel}>{installation.cancelRequested ? 'Cancellation requested' : 'Cancel setup'}</button>}</div>
    <p>Fairway creates files, installs dependencies, verifies them, publishes the folder, and saves the project registration.</p>
    {installation && <><div className="creation-log-toolbar"><h4>npm output</h4><label><input type="checkbox" checked={follow} onChange={event => setFollow(event.target.checked)} />Follow output</label></div>
      {view.limited && <p role="status">Earlier output was trimmed. Only recent output is shown.</p>}
      {error && <p role="alert">{error} <button type="button" className="text-link" onClick={() => setRetry(value => value + 1)}>Retry output</button></p>}
      <div ref={output} className="creation-output" role="region" aria-label="Installation output" tabIndex={0} onScroll={() => { const root = output.current; if (root) setFollow(root.scrollHeight - root.scrollTop - root.clientHeight < 32); }}>
        {!view.records.length && <p>No npm output recorded yet.</p>}{view.records.map(record => <div key={record.cursor} className={`creation-log-${record.stream}`}><span>{record.stream} · {new Date(record.time).toLocaleTimeString()}</span><pre>{record.text}</pre></div>)}
      </div></>}
  </section>;
}
