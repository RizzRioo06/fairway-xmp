import { isNodeProjectsError } from './types.ts';
import type { NodeLogRecord, NodeRunStatus } from './types.ts';

export function nodeError(error: unknown): string {
  const help: Record<string, string> = {
    conflict: 'Close this dialog, refresh projects, and inspect the folder again before retrying.',
    staleRun: 'The run changed. Refresh its status before choosing an action again.',
    runtimeUnavailable: 'Edit the project and select an installed Node version.',
    runtimeChanged: 'Refresh runtimes in Edit and select the installation again.',
    npmUnavailable: 'Repair or choose an installation that includes npm.',
    requirementConflict: 'Choose a Node/npm version satisfying all displayed requirements.',
    projectBusy: 'Wait for cleanup, or stop the current run before editing.',
  };
  if (isNodeProjectsError(error)) return [error.message, help[error.code]].filter(Boolean).join(' ');
  return error instanceof Error ? error.message : 'The operation could not be completed. Refresh and try again.';
}
export function activeRun(run?: NodeRunStatus) {
  return !!run && ['starting', 'running', 'stopping'].includes(run.state);
}
// Different run IDs are generations. Equal timestamps need an authoritative
// snapshot rather than guessing an order from random IDs.
export function mergeRun(current: NodeRunStatus | undefined, incoming: NodeRunStatus, authoritative = false): NodeRunStatus {
  if (!current) return incoming;
  if (current.runId === incoming.runId) return incoming.revision > current.revision ? incoming : current;
  if (incoming.createdAt > current.createdAt || (authoritative && incoming.createdAt === current.createdAt)) return incoming;
  return current;
}
export function localUrl(value: string): boolean {
  if (!value) return true;
  try {
    if (value.length > 2048 || !/^https?:\/\/(localhost|127\.0\.0\.1|\[::1\])(?::[0-9]+)?(?:[/?#]|$)/.test(value)) return false;
    const url = new URL(value);
    return ['http:', 'https:'].includes(url.protocol) && !url.username && !url.password
      && ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname) && url.port !== '0'
      && !/[\s\\]/.test(value);
  } catch { return false; }
}
export function appendLogs(current: NodeLogRecord[], incoming: NodeLogRecord[], runId: string, cursor: number) {
  const fresh = incoming.filter(row => row.runId === runId && row.cursor > cursor);
  const unique = new Map<number, NodeLogRecord>();
  for (const row of [...current, ...fresh]) unique.set(row.cursor, row);
  const records = [...unique.values()].sort((a, b) => a.cursor - b.cursor);
  let bytes = records.reduce((total, row) => total + new TextEncoder().encode(row.text).length, 0);
  let trimmed = false;
  while (records.length > 1024 || bytes > 256 * 1024) {
    bytes -= new TextEncoder().encode(records.shift()!.text).length; trimmed = true;
  }
  return { records, trimmed };
}
