// Node Projects wire contracts mirror src-tauri/src/node_projects.rs. Runtime choices
// are issued by native discovery; save requests never supply an executable.
export type NodeProjectRuntime = { nodePath: string; version: string };
export type NodeRuntimeChoice = NodeProjectRuntime & { id: string; source: string };
export type NodeProjectConfig = {
  id: string;
  name: string;
  root: string;
  script: string;
  runtime: NodeProjectRuntime;
  openUrl: string | null;
};
export type NodeProjectView = {
  config: NodeProjectConfig;
  rootAvailable: boolean;
  manifestAvailable: boolean;
  // Path/executable availability, not an execution-time version guarantee.
  runtimeAvailable: boolean;
};
export type NodeProjectsErrorCode =
  | 'unsupportedPlatform' | 'unavailable' | 'invalidProject' | 'invalidManifest'
  | 'confirmationRequired' | 'projectBusy' | 'runNotFound' | 'staleRun' | 'executionFailed' | 'logUnavailable'
  | 'runtimeChanged' | 'npmUnavailable' | 'environmentUnavailable'
  | 'requirementConflict' | 'unsupportedPackageManager'
  | 'runtimeUnavailable' | 'registryUnavailable' | 'conflict' | 'notFound' | 'saveFailed';
export type NodeProjectsError = { code: NodeProjectsErrorCode; message: string };
export type NodeProjectsSnapshot = {
  supported: boolean;
  revision: string | null;
  registryError: NodeProjectsError | null;
  projects: NodeProjectView[];
};
export type NodeProjectInspection = {
  root: string;
  name: string;
  scripts: Record<string, string>;
  packageManager: { name: string; declaration: string | null; lockfiles: string[]; npmSupported: boolean };
  versionHints: { nodeVersion: string | null; nvmrc: string | null; enginesNode: string | null };
  manifestRevision: string;
  warnings: string[];
  runtimes: NodeRuntimeChoice[];
  suggestedRuntimeId: string | null;
  runtimeError: string | null;
};
export type InspectNodeProjectRequest = { root: string };
export type SaveNodeProjectRequest = {
  config: {
    id: string | null;
    name: string;
    root: string;
    script: string;
    runtimeId: string;
    openUrl: string | null;
  };
  expectedRevision: string;
  manifestRevision: string;
};
export type RemoveNodeProjectRequest = { id: string; expectedRevision: string };
export type SaveNodeProjectResponse = { revision: string; project: NodeProjectConfig };
export type NodeProjectsChanged = { revision: string };

const errorCodes = new Set<NodeProjectsErrorCode>([
  'unsupportedPlatform', 'unavailable', 'invalidProject', 'invalidManifest',
  'confirmationRequired', 'projectBusy', 'runNotFound', 'staleRun', 'executionFailed', 'logUnavailable',
  'runtimeChanged', 'npmUnavailable', 'environmentUnavailable', 'requirementConflict', 'unsupportedPackageManager',
  'runtimeUnavailable', 'registryUnavailable', 'conflict', 'notFound', 'saveFailed',
]);

// Tauri rejects with the serialized Rust error object, not a JavaScript Error.
export function isNodeProjectsError(value: unknown): value is NodeProjectsError {
  if (!value || typeof value !== 'object') return false;
  const error = value as Record<string, unknown>;
  return typeof error.message === 'string' && typeof error.code === 'string'
    && errorCodes.has(error.code as NodeProjectsErrorCode);
}

export type PrepareNodeRuntimeRequest = {
  id: string;
  expectedRevision: string;
  manifestRevision: string;
};
export type NodeRuntimePreparation = {
  projectId: string;
  registryRevision: string;
  manifestRevision: string;
  runtime: NodeProjectRuntime;
  npm: { cliPath: string; version: string };
  // Validation receipt, never an execution token or serialized environment.
  runtimeRevision: string;
};

export type NodeRunState = 'starting' | 'running' | 'stopping' | 'stopped' | 'exited' | 'failed';
export type NodeRunStatus = {
  projectId: string;
  runId: string;
  revision: number;
  state: NodeRunState;
  createdAt: number;
  startedAt: number | null;
  endedAt: number | null;
  exitCode: number | null;
  signal: number | null;
  error: NodeProjectsError | null;
  forcedCleanup: boolean;
  runtime: NodeProjectRuntime;
};
export type StartNodeProjectRequest = PrepareNodeRuntimeRequest & { confirmed: true };
export type StopNodeProjectRequest = { id: string; runId: string };
export type RestartNodeProjectRequest = StartNodeProjectRequest & { runId: string };
export type NodeRunsSnapshot = { supported: boolean; runs: NodeRunStatus[] };
export type NodeLogRecord = { runId: string; cursor: number; time: number; stream: 'stdout' | 'stderr' | 'system'; text: string };
export type ReadNodeLogsRequest = { id: string; runId: string; afterCursor: number };
export type NodeLogBatch = { records: NodeLogRecord[]; nextCursor: number; truncated: boolean; eof: boolean; error: string | null };

export function isNodeRunStatus(value: unknown): value is NodeRunStatus {
  if (!value || typeof value !== 'object') return false;
  const run = value as Record<string, unknown>;
  const identity = (id: unknown) => typeof id === 'string' && /^[a-f0-9]{32}$/i.test(id);
  const timestamp = (time: unknown) => typeof time === 'number' && Number.isSafeInteger(time) && time >= 0;
  const optionalTime = (time: unknown) => time === null || timestamp(time);
  const optionalInteger = (number: unknown) => number === null || (typeof number === 'number' && Number.isSafeInteger(number));
  const runtime = run.runtime as Record<string, unknown> | null;
  return identity(run.projectId) && identity(run.runId)
    && typeof run.revision === 'number' && Number.isSafeInteger(run.revision) && run.revision >= 1
    && typeof run.state === 'string' && ['starting', 'running', 'stopping', 'stopped', 'exited', 'failed'].includes(run.state)
    && timestamp(run.createdAt) && optionalTime(run.startedAt) && optionalTime(run.endedAt)
    && optionalInteger(run.exitCode) && optionalInteger(run.signal)
    && (run.error === null || isNodeProjectsError(run.error)) && typeof run.forcedCleanup === 'boolean'
    && !!runtime && typeof runtime === 'object' && typeof runtime.nodePath === 'string' && typeof runtime.version === 'string';
}
