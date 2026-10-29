import { isCreationError } from './types.ts';
import type { CreationJobStatus, NodeCreationPlan } from './types.ts';

export function creationError(error: unknown): string {
  const help: Record<string, string> = {
    invalidName: 'Use 1–64 lowercase letters, digits or hyphens, starting with a letter.',
    destinationExists: 'Choose a new project name or another parent folder. Existing files will be kept.',
    runtimeUnavailable: 'Refresh runtimes and select an installed Node.js version.',
    runtimeChanged: 'Check the setup again with the current runtime.',
    npmUnavailable: 'Choose or repair a Node installation that includes npm.',
    requirementConflict: 'Choose an installed Node/npm pair that meets the template requirements.',
    invalidLockfile: 'Refresh the catalog and review a new setup. No unpinned installation will be attempted.',
    staleInstallation: 'Setup changed. Refresh its status before trying the action again.',
    cancellationTooLate: 'Publication has begun. Keep the project and wait for the result.',
    registrationFailed: 'Your generated folder is kept. Retry Registration after resolving the registry error.',
    recoveryUnsafe: 'Files were kept for inspection. Review a fresh setup in a new destination.',
  };
  if (isCreationError(error)) return [error.message, help[error.code]].filter(Boolean).join(' ');
  return error instanceof Error ? error.message : 'Could not complete this request. Refresh setup status before trying again.';
}
export const needsReview = (error: unknown) => isCreationError(error) && ['planExpired', 'planStale', 'planUnavailable', 'planTampered', 'catalogChanged', 'runtimeChanged'].includes(error.code);
export function nameError(value: string): string | null {
  return /^[a-z][a-z0-9-]{0,63}$/.test(value) && !value.endsWith('-') ? null : 'Use 1–64 lowercase letters, digits or hyphens. Start with a letter and end with a letter or digit.';
}
export const planExpired = (plan: NodeCreationPlan, now: number) => now >= plan.expiresAt;
export const projectName = (job: CreationJobStatus) => job.destination.split(/[\\/]/).filter(Boolean).at(-1) ?? 'Node project';
export function complete(job: CreationJobStatus) {
  return job.phase === 'completed' && !!job.projectId && (!job.installation || job.installation.state === 'completed');
}
export function mergeJob(current: CreationJobStatus | undefined, incoming: CreationJobStatus): CreationJobStatus {
  if (!current || current.jobId !== incoming.jobId) return incoming;
  if (incoming.revision < current.revision) return current;
  if (current.installation && incoming.installation && (current.installation.jobId !== incoming.installation.jobId
    || current.installation.runId !== incoming.installation.runId || incoming.installation.revision < current.installation.revision)) return current;
  return incoming;
}
export function recovery(job: CreationJobStatus): 'finalize' | 'retryRegistration' | null {
  if (job.active || complete(job)) return null;
  if (job.installation) return job.installation.ciCompleted && job.installation.state !== 'completed' ? 'finalize' : null;
  return job.phase === 'needsRegistration' ? 'retryRegistration' : null;
}
export function phaseLabel(job: CreationJobStatus): string {
  if (complete(job)) return 'Project ready';
  if (job.installation && ['failed', 'cancelled', 'interrupted'].includes(job.installation.state)) return { failed: 'Setup failed', cancelled: 'Setup cancelled', interrupted: 'Setup interrupted' }[job.installation.state as 'failed' | 'cancelled' | 'interrupted'];
  if (job.installation?.cancelRequested) return 'Cancelling and cleaning up';
  if (job.phase === 'needsRegistration') return 'Registering project';
  if (job.phase === 'publishing') return 'Publishing project';
  const labels = { preparing: 'Generating project files', installing: 'Installing dependencies and cleaning up npm', verifying: 'Verifying project files and dependencies', publishing: 'Publishing project', registering: 'Registering project', complete: 'Finishing setup' };
  if (job.installation?.progress) return labels[job.installation.progress];
  return job.phase === 'staging' || job.phase === 'prepared' ? 'Generating project files' : job.installation?.state === 'installing' ? 'Native installation in progress' : 'Checking native setup status';
}
