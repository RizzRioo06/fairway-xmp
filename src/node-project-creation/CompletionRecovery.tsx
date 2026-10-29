import { complete, creationError, projectName, recovery } from './model';
import type { CreationJobStatus } from './types';
export default function CompletionRecovery({ job, pending, onRecover, onNew, onView }: { job: CreationJobStatus; pending: boolean; onRecover: () => void; onNew: () => void; onView: () => void }) {
  const ready = complete(job), action = recovery(job), error = job.installation?.error ?? job.error;
  if (job.active && !ready) return null;
  return <section className="creation-result" aria-label="Setup result">
    <h4>{ready ? `${projectName(job)} is registered${job.installation ? ' and ready' : ''}` : 'Your files are kept'}</h4>
    <p><code>{job.destination}</code></p>
    {ready ? <><p>{job.installation ? 'Dependencies were installed and verified. ' : 'Dependencies were not installed by this setup. '}No server has been started. Use Start on the project card when you are ready; the existing script confirmation will appear.</p><button type="button" className="primary" onClick={onView}>View project</button></> : <>
      {error && <p className="creation-error" role="alert">{creationError(error)}</p>}
      {job.installation?.exitCode !== null && job.installation?.exitCode !== undefined && <p>npm exited with code {job.installation.exitCode}.</p>}
      <p>{job.installation?.state === 'interrupted' ? 'This setup was interrupted. Fairway has not restarted the setup or any server automatically. ' : ''}{job.phase === 'needsRegistration' || job.projectId ? 'The published project folder is preserved. Retry Registration after resolving the error.' : 'Any staging files are preserved. An unfinished installation will not be published automatically.'}</p>
      {action && <button type="button" className="primary" disabled={pending} onClick={onRecover}>{pending ? 'Recovering…' : job.phase === 'needsRegistration' || job.projectId || !job.installation ? 'Retry Registration' : 'Retry finishing setup'}</button>}
      <p>Published projects and uncertain dependency files are never deleted here. To retry an incomplete installation, review a fresh setup and destination. A new setup creates a separate project; it will not copy or overwrite your previous files.</p>
      <button type="button" className="secondary" disabled={pending} onClick={onNew}>Review a new setup</button>
    </>}
  </section>;
}
